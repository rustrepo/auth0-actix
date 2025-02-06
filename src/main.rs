use actix_web::{HttpRequest, web, App, HttpResponse, HttpServer, Responder, delete, get, post, put};
use darkbird::document::{Document, FullText, Indexer, MaterializedView, Range, RangeField, Tags};
use darkbird::{Storage, StorageType, Options};
use mongodb::{bson::{doc, oid::ObjectId, Document as BsonDocument}, options::ClientOptions, Client, Collection};
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use std::env;
use dotenv::dotenv;
use actix_web::http::header::HeaderMap;
//use base64::engine::general_purpose::STANDARD as base64_engine;
//use base64::Engine;
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
//use reqwest::{Url, Response};
use std::collections::HashMap;




type Pid = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    fullname: String,
}

impl Document for User {}

impl Indexer for User {
    fn extract(&self) -> Vec<String> {
        vec![self.fullname.clone()]
    }
}

impl Tags for User {
    fn get_tags(&self) -> Vec<String> {
        vec![]
    }
}

impl Range for User {
    fn get_fields(&self) -> Vec<RangeField> {
        vec![]
    }
}

impl MaterializedView for User {
    fn filter(&self) -> Option<String> {
        None
    }
}

impl FullText for User {
    fn get_content(&self) -> Option<String> {
        None
    }
}

struct AppState {
    cache: Arc<Storage<Pid, User>>,  
    mongo_collection: Arc<Mutex<Collection<BsonDocument>>>,  
}



#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: usize,
    // Add other claims as needed
}

async fn validate_token(token: &str) -> Result<Claims, String> {
    let domain = env::var("AUTH0_DOMAIN").expect("AUTH0_DOMAIN not set");
    let audience = env::var("AUTH0_AUDIENCE").expect("AUTH0_AUDIENCE not set");

    let jwks_url = format!("https://{}/.well-known/jwks.json", domain);
    let jwks: HashMap<String, serde_json::Value> = reqwest::get(&jwks_url)
        .await
        .map_err(|_| "Failed to fetch JWKS".to_string())?
        .json()
        .await
        .map_err(|_| "Failed to parse JWKS".to_string())?;

    let jwk = jwks.get("keys").and_then(|keys| keys.get(0)).ok_or("No keys found in JWKS".to_string())?;
    let n = jwk.get("n").and_then(|n| n.as_str()).ok_or("Invalid key format".to_string())?;
    let e = jwk.get("e").and_then(|e| e.as_str()).ok_or("Invalid key format".to_string())?;

    let decoding_key = DecodingKey::from_rsa_components(n, e).map_err(|_| "Invalid key components".to_string())?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[audience]);

    decode::<Claims>(token, &decoding_key, &validation)
        .map(|data| data.claims)
        .map_err(|err| format!("Invalid token: {}", err))
}

async fn check_auth(headers: &HeaderMap) -> bool {
    if let Some(auth_header) = headers.get("Authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("Bearer ") {
                let token = auth_str.trim_start_matches("Bearer ");
                return validate_token(token).await.is_ok();
            }
        }
    }
    false
}


#[post("/users")]
async fn create_user(data: web::Data<AppState>, user: web::Json<User>, req: HttpRequest) -> impl Responder {
    if !check_auth(req.headers()).await {
        return HttpResponse::Unauthorized().body("Unauthorized");
    }

    // Rest of the function remains the same
    let pid = ObjectId::new().to_hex();
    let user = user.into_inner();
    
    let user_doc = doc! { 
        "_id": &pid, 
        "fullname": &user.fullname 
    };

    if data.mongo_collection.lock().await.insert_one(user_doc, None).await.is_err() {
        return HttpResponse::InternalServerError().body("Error saving to MongoDB");
    }

    if data.cache.insert(pid.clone(), user).await.is_err() {
        return HttpResponse::InternalServerError().body("Error caching user in Darkbird");
    }

    HttpResponse::Ok().json(pid)
}

#[get("/users/{pid}")]
async fn get_user(data: web::Data<AppState>, pid: web::Path<String>, req: HttpRequest) -> impl Responder {
    if !check_auth(req.headers()).await {
        return HttpResponse::Unauthorized().body("Unauthorized");
    }

    // Rest of the function remains the same
    let pid = pid.into_inner();

    if let Some(user_ref) = data.cache.lookup(&pid) {
        return HttpResponse::Ok().json(user_ref.value().clone());
    }

    let filter = doc! { "_id": &pid };
    if let Some(user_doc) = data.mongo_collection.lock().await.find_one(filter, None).await.unwrap() {
        if let Ok(user) = bson::from_document::<User>(user_doc) {
            let _ = data.cache.insert(pid.clone(), user.clone()).await;
            return HttpResponse::Ok().json(user);
        }
    }

    HttpResponse::NotFound().body("User not found")
}

#[put("/users/{pid}")]
async fn update_user(data: web::Data<AppState>, pid: web::Path<String>, user: web::Json<User>, req: HttpRequest) -> impl Responder {
    if !check_auth(req.headers()).await {
        return HttpResponse::Unauthorized().body("Unauthorized");
    }

    // Rest of the function remains the same
    let pid = pid.into_inner();
    let user = user.into_inner();
    let filter = doc! { "_id": &pid };
    let update = doc! { "$set": { "fullname": &user.fullname } };

    if data.mongo_collection.lock().await.update_one(filter.clone(), update, None).await.is_err() {
        return HttpResponse::InternalServerError().body("Error updating MongoDB");
    }
    
    if data.cache.insert(pid.clone(), user.clone()).await.is_err() {
        return HttpResponse::InternalServerError().body("Error updating cache in Darkbird");
    }

    HttpResponse::Ok().json("User updated successfully")
}

#[delete("/users/{pid}")]
async fn delete_user(data: web::Data<AppState>, pid: web::Path<String>, req: HttpRequest) -> impl Responder {
    if !check_auth(req.headers()).await {
        return HttpResponse::Unauthorized().body("Unauthorized");
    }

    // Rest of the function remains the same
    let pid = pid.into_inner();
    let filter = doc! { "_id": &pid };

    if data.mongo_collection.lock().await.delete_one(filter, None).await.is_err() {
        return HttpResponse::InternalServerError().body("Error deleting from MongoDB");
    }
    
    if data.cache.remove(pid).await.is_err() {
        return HttpResponse::InternalServerError().body("Error removing from cache in Darkbird");
    }

    HttpResponse::Ok().json("User deleted successfully")
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    
    let path = ".";
    let storage_name = "blackbird";
    let total_page_size = 1000;
    let stype = StorageType::RamCopies;
    let ops = Options::new(path, storage_name, total_page_size, stype, true);
    let cache = Arc::new(Storage::<Pid, User>::open(ops).await.unwrap());

    let mongodb_uri = env::var("MONGODB_URI").expect("MONGODB_URI not set");
    let mongodb_db = env::var("MONGODB_DATABASE").expect("MONGODB_DATABASE not set");
    let mongodb_collection = env::var("MONGODB_COLLECTION").expect("MONGODB_COLLECTION not set");

    let client_options = ClientOptions::parse(&mongodb_uri).await.unwrap();
    let client = Client::with_options(client_options).unwrap();
    let db = client.database(&mongodb_db);
    let collection = db.collection::<BsonDocument>(&mongodb_collection);
    let mongo_collection = Arc::new(Mutex::new(collection));

    let app_state = web::Data::new(AppState { cache, mongo_collection });

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .service(create_user)
            .service(get_user)
            .service(update_user)
            .service(delete_user)
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}