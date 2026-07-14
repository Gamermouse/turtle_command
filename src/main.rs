use rocket::form::Form;
use rocket::fs::{FileServer, NamedFile};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::{path::Path};
use uuid::{Uuid};
use std::{vec, fs};
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

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize, Clone, Copy)]
struct Coordinate {
    x: i32,
    y: i32,
    z: i32
}

impl Coordinate {
    /// Creates a coordinate using the x, y, and z inputs
    fn new(x: i32, y: i32, z: i32) -> Self {
        Self {x, y, z}
    }

    fn world_to_local_coords(&self) -> Self {
        Coordinate::new(self.x & 0xF, self.y & 0xF, self.z & 0xF)
    }

    fn world_to_chunk_coords(&self) -> Self {
        Coordinate::new(self.x >> 4, self.y >> 4, self.z >> 4)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
#[serde(untagged)]
enum BlockStateData {
    Bool(bool),
    Integer(i32),
    String(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
struct BlockData {
    name: String,
    states: HashMap<String, BlockStateData>
}

#[derive(Debug, PartialEq, Eq)]
#[derive(Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
struct Chunk {
    coordinates: Coordinate,
    block_data: Vec<Vec<Vec<BlockData>>>
}

impl Chunk {
    /// Creates a 16x16x16 vector filled with air
    fn new(coordinates: &Coordinate) -> Self {
        Self {
            coordinates: *coordinates,
            block_data: vec![vec![vec![BlockData {name: "minecraft:air".to_string(), states: HashMap::new() }; 16]; 16]; 16]
        }
    }

    /// Sets a block in the chunk to the input
    fn set_block(&mut self, coordinate: &Coordinate, block: &BlockData) {
        self.block_data[coordinate.x as usize][coordinate.y as usize][coordinate.z as usize] = block.clone();
    }

    fn get_name(coords: &Coordinate) -> String {
       coords.x.to_string() + "_" + &coords.y.to_string() + "_" + &coords.z.to_string()
    }

    /// Saves this chunks data to the given path with the correct name
    fn save<P: AsRef<Path>>(&self, path: &P) {
        let path = path.as_ref();
        let file_name = Self::get_name(&self.coordinates);
        let chunk_file = std::fs::File::create(path.join(file_name)).unwrap();

        let chunk_file = lz4_flex::frame::FrameEncoder::new(chunk_file).auto_finish();

        ciborium::into_writer(self, chunk_file).unwrap();
    }

    /// Creates a new chunk object from a path that is given
    fn load<P: AsRef<Path>>(path: &P, coordinates: &Coordinate) -> Option<Self> {
        let path = path.as_ref();
        let file_name = Self::get_name(coordinates);
        let reader = std::fs::File::open(path.join(file_name)).ok()?;

        let reader = lz4_flex::frame::FrameDecoder::new(reader);

        ciborium::from_reader(reader).unwrap()
    }

    fn load_or_new<P: AsRef<Path>>(path: &P, coordinates: &Coordinate) -> Self {
        let path = path.as_ref();
        if let Some(chunk) = Self::load(&path, coordinates){
            chunk
        } else {
            Self::new(coordinates)
        }
    }
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

// Registers a turtle's data, used to update turtle data files currently
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

// Recieves blocks from the turtles to be stored in chunk files
fn ws_receive_blocks(reg_data: &String) {
    let blocks: Vec<(BlockData, Coordinate)> = json::from_str(&reg_data).unwrap();

    for block in blocks.iter() {
        let world_coords = Coordinate::new(block.1.x, block.1.y, block.1.z);
        let chunk_coords = world_coords.world_to_chunk_coords();

        let mut chunk = Chunk::load_or_new(&WORLD_FOLDER, &chunk_coords);
        let local_coords = world_coords.world_to_local_coords();

        chunk.set_block(&local_coords, &BlockData { name: block.0.name.clone(), states: block.0.states.clone() });
        chunk.save(&WORLD_FOLDER);
    }
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
                    println!("Recieved unexpected websocket result. Ignoring.");
                    continue
                };

                // Deserializes the json into a TurtleReadable object
                // It is likely the case that message.data is another json string, which we can then decode in the respective function
                let message: Result<TurtleReadable, json::serde_json::Error> = json::from_str(&message);

                // We make sure that the json deserialized properly
                match message {
                    Ok(message) => {
                        let _ = match message.instruction.as_str()  {
                        "register" => ws_register(&message.data, &connections),
                        "sendBlocks" => ws_receive_blocks(&message.data),

                        // Unexpected result, we just ignore it
                        _ => {
                            println!("Recieved unknown websocket result. Ignoring.");
                            continue
                        }
                        };
                    }

                    Err(_) => println!("Error parsing json. Ignoring.")
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
const WORLD_FOLDER: &'static str = "world_data";

#[launch]
fn rocket() -> _ {
    // Creates the world data folder if it doesnt exist
    let path = PathBuf::from(WORLD_FOLDER);
    let _ = fs::create_dir(&path);
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
// See if you can move over the pathfinding and world exploration code from the turtleswarm project
// Add a login system so only people who are authorized can send commands to turtles (maybe)