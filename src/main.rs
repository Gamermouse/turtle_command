use rocket::form::Form;
use rocket::fs::{FileServer, NamedFile, TempFile};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::OsString;
use std::sync::{Arc, Mutex};
use std::{path::Path};
use uuid::{Uuid};
use std::{fs, vec};
use rocket::{http::Status, serde::json, State};
use rocket::request::{FromRequest, Request, Outcome};
use rocket::tokio::sync::mpsc;
use rocket_ws as ws;
#[macro_use] extern crate rocket;

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

// Data structured so the turtle can read and parse it
// Also the data structure sent from the turtle
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(crate = "rocket::serde")]
struct TurtleReadable {
    instruction: String,
    data: String
}

impl TurtleReadable {
    fn new(instruction: &str, data: &str) -> Self {
        TurtleReadable { instruction: instruction.to_string(), data: data.to_string() }
    }

    fn serialize(self) -> json::Json<TurtleReadable> {
        json::Json(self)
    }

    fn to_ws_message(self) -> ws::Message {
        ws::Message::Text(json::to_string(&self).unwrap())
    }
}

// NOTE: Pattially created with AI
// Registry that maps a turtle's id to a sender half of an mpsc channel.
// Any route (e.g. web_command) can grab this shared, managed state and push
// a message onto a specific turtle's channel. The websocket task for that
// turtle is the one reading from the *receiver* half and forwarding
// the message out over the actual websocket.
struct TurtleConnections {
    senders: Mutex<HashMap<u16, mpsc::UnboundedSender<ws::Message>>>
}

impl TurtleConnections {
    fn new() -> Self {
        TurtleConnections { senders: Mutex::new(HashMap::new()) }
    }

    fn register(&self, id: u16, sender: mpsc::UnboundedSender<ws::Message>) {
        self.senders.lock().unwrap().insert(id, sender);
    }

    fn unregister(&self, id: u16) {
        self.senders.lock().unwrap().remove(&id);
    }

    // Returns true if the message was successfully queued for delivery
    fn send_to(&self, id: u16, message: ws::Message) -> bool {
        if let Some(sender) = self.senders.lock().unwrap().get(&id) {
            sender.send(message).is_ok()
        } else {
            false
        }
    }

    fn get_connected_ids(&self) -> Vec<u16> {
        let senders_vec = self.senders.lock().unwrap().keys().copied().collect();
        senders_vec
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
            Some(contents) => {
                let mut new_inventory = Inventory {size: size, slots: (contents) };
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
    id: u16,
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
    id: u16,
    connected: bool,
    inventory_contents: Option<Vec<Option<Slot>>>,
    equipped_left: Option<Slot>,
    equipped_right: Option<Slot>,
    coordinates: Coordinate,
    fuel: i16
}

fn ws_register(reg_data: &String, connections: &Arc<TurtleConnections>) {
    let reg_data: TurtleRegistrationData = json::from_str(&reg_data).unwrap();

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
    let response = TurtleReadable::new("status", "successful").to_ws_message();
    connections.send_to(reg_data.id, response);
}

// NOTE: Partially created with AI
// Starts a websocket connection with a turtle.
// Turtles connect with their id in the query string, e.g. `/websocket?id=5`
// We register an mpsc sender for that id in the shared TurtleConnections state, then run two loops concurrently:
//   - outgoing: anything pushed onto the mpsc channel (e.g. from web_command)
//     gets forwarded out over the actual websocket to the turtle
//   - incoming: anything the turtle sends back gets read and can be handled
//     (logged, parsed, used to update turtle state, etc.)
#[get("/websocket?<id>")]
fn websocket(ws: ws::WebSocket, id: u16, connections: &State<Arc<TurtleConnections>>, _key: ApiKey) -> ws::Channel<'static> {
    use rocket::futures::{SinkExt, StreamExt};

    let connections = connections.inner().clone();
    let (tx, mut rx) = mpsc::unbounded_channel::<ws::Message>();
    connections.register(id, tx);

    ws.channel(move |stream| Box::pin(async move {
        let (mut sink, mut source) = stream.split();

        let outgoing = async {
            while let Some(msg) = rx.recv().await {
                if sink.send(msg).await.is_err() {
                    break;
                }
            }
        };

        let incoming = async {
            while let Some(message) = source.next().await {
                // Verify that the message is ok
                let Ok(message) = message else {
                    break
                };

                // Makes sure that it is a text input
                let ws::Message::Text(message) = message else {
                    // Unexpected result, we just ignore it
                    continue
                };

                // Deserializes the json into a TurtleReadable object
                // It is likely the case that message.data is another json string, which we can then decode in the respective function
                let message: Result<TurtleReadable, json::serde_json::Error> = json::from_str(&message);

                // We make sure that the json deserialized properly
                match message {
                    Ok(m) => {
                        let _ = match m.instruction.as_str()  {
                        "register" => ws_register(&m.data, &connections),

                        // Unexpected result, we just ignore it
                        _ => continue
                        };
                    }

                    Err(_) => println!("Error parsing json. Ignoring request.")
                }

            }
        };

        rocket::tokio::select! {
            _ = outgoing => {},
            _ = incoming => {},
        }

        connections.unregister(id);

        Ok(())
    }))
}

// Handles the front page
#[get("/")]
async fn index() -> Result<NamedFile, std::io::Error> {
    NamedFile::open("frontend/front_page.html").await
}

#[derive(FromForm, Debug)]
struct WebCommand<'r> {
    id: u16,
    kind: &'r str,
    data: &'r str,
}

// Forwards a form submission to the specific turtle's open websocket connection, if one exists.
#[post("/web_command", data = "<command>")]
fn web_command(command: Form<WebCommand<'_>>, connections: &State<Arc<TurtleConnections>>) -> Status {

    let message = TurtleReadable::new(command.kind, command.data).to_ws_message();

    if connections.send_to(command.id, message) {
        Status::Ok
    } else {
        // No open websocket for that turtle id
        Status::NotFound
    }
}

// Handles the control test page
#[get("/control")]
async fn control() -> Result<NamedFile, std::io::Error> {
    NamedFile::open("frontend/control_test.html").await
}

// We send back json containing data the user may need to manage turtles
// TODO: Make this live updating in the future
#[get("/connected_ids")]
fn connected_ids(connections: &State<Arc<TurtleConnections>>) -> json::Json<Vec<u16>> {
    let connections = connections.get_connected_ids();
    json::Json(connections)
}

const LUA_FOLDER: &'static str = "lua";

#[launch]
fn rocket() -> _ {
    // Creates a new API key if there isn't one
    ApiKey::load_or_new();
    rocket::build()
    // Initializes the turtle connection manager
    .manage(Arc::new(TurtleConnections::new()))
    // This hosts all the files in the lua folder, so if we recieve a get request that has /lua/filepath it will go to that filepath
    .mount("/".to_owned()+LUA_FOLDER, FileServer::from(LUA_FOLDER.to_owned()+"/"))
    .mount("/", routes![websocket, index, control, web_command, connected_ids])
}

// TODO:
// Implement pings on the rust side to make sure the connection is active
// Implement commands on the rust side to control the turtle (just the basics like moving)
// See if you can move over the pathfinding and world exploration code from the turtleswarm project
// Add a login system so only people who are authorized can send commands to turtles (maybe)