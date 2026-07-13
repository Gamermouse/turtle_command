use rocket::fs::FileServer;
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::{path::Path};
use uuid::{Uuid};
use std::{fs, vec};

use rocket::{http::Status, serde::json};
use rocket::request::{FromRequest, Request, Outcome};
use rocket_ws as ws;
#[macro_use] extern crate rocket;

#[derive(Serialize)]
struct LuaReadableResponse {
    kind: String,
    payload: String
}

impl LuaReadableResponse {
    fn to_string(&self) -> String {
        format!("{} {}",self.kind,self.payload)
    }
}

#[derive(Debug)]
enum ApiKeyError {
    Missing,
    Invalid,
}

struct ApiKey {
    uuid: uuid::Uuid
}

impl ApiKey {
    // Creates a new UUID for the API key and saves the file
    fn new() -> Self {
        let new_uuid = Uuid::new_v4();
        fs::write("api_key.txt", new_uuid.to_string()).expect("Should be able to write to `api_key.txt`");

        Self {
            uuid: new_uuid
        }
    }

    // Creates a new UUID object from the file
    fn load() -> Self {
        let  data = fs::read_to_string("api_key.txt").expect("Should be able to read `api_key.txt`");
        return ApiKey { uuid: Uuid::parse_str(&data).unwrap() };
    }

    // Either loads the file or creates a new UUID if there isnt one
    fn load_or_new() -> Self {
        if Path::new("api_key.txt").exists() {
            return Self::load();
        } else {
            Self::new()
        }
    }

    fn equal_to_string(&self, check_with: &str) -> bool {
        if Uuid::parse_str(check_with).is_err() {
            return false
        }

        self.uuid == Uuid::parse_str(check_with).unwrap()
    }
}

// Manages dispatching turtles, tracking them, and resolving issues
// Should also handle keeping data up to date, such as pinging turtles to make sure they are still connected
struct TurtleManager {
    turtles: Vec<Turtle>
}

impl TurtleManager {
    // Loads all the turtles from the turtles/ directory and returns a TurtleManager object
    fn load() -> Self {
        let mut turtle_list = vec![];
        let turtle_iter = std::fs::read_dir("turtles/").unwrap();

        for path in turtle_iter {
            turtle_list.push(Turtle::load(path.unwrap().file_name()));
        }

        TurtleManager{ turtles: turtle_list}
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Slot {
    name: String,
    count: i8
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct Inventory {
    size: i32,
    slots: Vec<Option<Slot>>
}

impl Inventory {
    fn new(size: i32, contents: Option<Vec<Option<Slot>>>) -> Self {
        match contents {
            Some(contents_verified) => {
                let mut new_inventory = Inventory {size: size, slots: (contents_verified) };
                let deficit = new_inventory.size - (new_inventory.slots.len() as i32);

                if deficit > 0 {
                    // Fills the rest of the array with None if it isnt full

                    let mut slice: Vec<Option<Slot>> = vec![None;deficit as usize];
                    new_inventory.slots.append(&mut slice);
                }
                new_inventory
            }

            None => Self::new_empty(size)
        }
    }

    fn new_empty(size: i32) -> Self {
        Inventory { size: size, slots: vec![None; size.try_into().unwrap()] }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize, Clone)]
struct Coordinate {
    x: i32,
    y: i32,
    z: i32
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct Turtle {
    id: i16,
    connected: bool,
    inventory: Inventory,
    equipped_left: Option<Slot>,
    equipped_right: Option<Slot>,
    coordinates: Coordinate,
    fuel: i16
}

impl Turtle {
    // Saves itself to a file in turtles/ with the name being its id
    fn save(&self) {
        let string_self = json::to_pretty_string(&self).unwrap();
        if !fs::exists("turtles/").unwrap() {
            fs::create_dir("turtles/").unwrap();
        }
        fs::write(format!("turtles/{}.json",self.id), string_self).expect(&format!("Should be able to write to `turtles/{}.json`",self.id));
    }

    fn load(filepath: OsString) -> Self {
        let  data = fs::read_to_string(&filepath).expect(&format!("Should be able to read `{}`",filepath.display()));
        let new_self: Turtle = json::from_str(&data).unwrap();

        new_self
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for ApiKey {
    type Error = ApiKeyError;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        /// Returns true if `key` is a valid API key
        fn is_valid(key: &str) -> bool {
            ApiKey::load().equal_to_string(key)
        }

        match req.headers().get_one("api_key") {
            None => Outcome::Error((Status::BadRequest, ApiKeyError::Missing)),
            Some(key) if is_valid(key) => Outcome::Success(ApiKey::load()),
            Some(_) => Outcome::Error((Status::BadRequest, ApiKeyError::Invalid)),
        }
    }
}

#[derive(Deserialize, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct TurtleRegistrationData {
    id: i16,
    connected: bool,
    inventory_contents: Option<Vec<Option<Slot>>>,
    equipped_left: Option<Slot>,
    equipped_right: Option<Slot>,
    coordinates: Coordinate,
    fuel: i16
}

// Registers a turtle in the network
#[post("/register", data = "<reg_data>")]
fn register(reg_data: json::Json<TurtleRegistrationData>, key: ApiKey) -> String {
    dbg!(&reg_data);

    let new_turtle = Turtle {
        id: reg_data.id,
        connected: reg_data.connected,
        inventory: Inventory::new(16, reg_data.inventory_contents.clone()),
        equipped_left: reg_data.equipped_left.clone(),
        equipped_right: reg_data.equipped_right.clone(),
        coordinates: reg_data.coordinates.clone(),
        fuel: reg_data.fuel
    };

    Turtle::save(&new_turtle);
    LuaReadableResponse {kind: "status".to_string(), payload: "successful".to_string()}.to_string()
}

// Starts a websocket connection with a turtle
// Turtles will likely do this right when they boot up
#[get("/websocket")]
fn websocket(ws: ws::WebSocket, key: ApiKey) -> ws::Channel<'static> {
    use rocket::futures::{SinkExt, StreamExt};

    ws.channel(move |mut stream| Box::pin(async move {
        while let Some(message) = stream.next().await {
            let _ = stream.send(rocket_ws::Message::Text("Test".to_string())).await;
        }

        Ok(())
    }))
}

const LUA_FOLDER: &'static str = "lua";

#[launch]
fn rocket() -> _ {
    // Creates a new API key if there isn't one
    ApiKey::load_or_new();
    rocket::build().mount("/", routes![register, websocket])
    
    // This hosts all the files in the lua folder, so if we recieve a get request that has /lua/filepath it will go to that filepath
    .mount("/".to_owned()+LUA_FOLDER, FileServer::from("lua/"))
}