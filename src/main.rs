#![feature(plugin, custom_derive)]
#![plugin(rocket_codegen)]

extern crate arrayvec;
extern crate rocket;
extern crate rocket_contrib;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

use rocket::State;
use rocket_contrib::Json;

use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::io::{self, Read};
use std::fs::File;
use std::path::{Path, PathBuf};

#[get("/users/<id>")]
fn users(id: u32, storage: State<Storage>) -> Option<Json<User>> {
    storage.users.get(&id).map(|entity| Json(entity.clone()))
}

#[derive(Serialize, Deserialize, FromForm)]
struct UsersVisitsParams {
    from_date: Option<i32>,
    to_date: Option<i32>,
    country: Option<String>,
    to_distance: Option<u32>,
}

#[get("/users/<id>/visits")]
fn users_visits_no_params(
    id: u32,
    storage: State<Storage>,
) -> Option<Json<HashMap<String, Vec<Visit>>>> {
    users_visits(id, None, storage)
}

#[get("/users/<id>/visits?<params>")]
fn users_visits(
    id: u32,
    params: Option<UsersVisitsParams>,
    storage: State<Storage>,
) -> Option<Json<HashMap<String, Vec<Visit>>>> {
    let all_visits = &storage.visits;
    let user_visits = all_visits
        .iter()
        .map(|(_, v)| v)
        .cloned()
        .filter(|v| v.user == id);

    let result_visits = if let Some(params) = params {
        let from_date_visits = user_visits.filter(|v| if let Some(from_date) = params.from_date {
            if from_date > v.visited_at {
                true
            } else {
                false
            }
        } else {
            true
        });

        let to_date_visits = from_date_visits.filter(|v| if let Some(to_date) = params.to_date {
            if to_date > v.visited_at {
                true
            } else {
                false
            }
        } else {
            true
        });

        let country_visits = to_date_visits.filter(|v| if let Some(ref country) = params.country {
            // FIXME: get rid of unwrap
            let reference_country = &storage.locations.get(&v.location).unwrap().country;

            if country == reference_country {
                true
            } else {
                false
            }
        } else {
            true
        });

        let to_distance_visits =
            country_visits.filter(|v| if let Some(to_distance) = params.to_distance {
                // FIXME: get rid of unwrap
                let reference_distance = &storage.locations.get(&v.location).unwrap().distance;

                if to_distance < *reference_distance {
                    true
                } else {
                    false
                }
            } else {
                true
            });

        let final_visits: Vec<_> = to_distance_visits.collect();
        final_visits
    } else {
        user_visits.collect()
    };

    let mut response = HashMap::new();
    response.insert("visits".to_owned(), result_visits);
    Some(Json(response))
}

#[post("/users/<id>")]
fn users_update(id: u32) -> String {
    format!("user {} update", id)
}

#[post("/users/new")]
fn users_new() -> String {
    format!("user new")
}

#[get("/locations/<id>")]
fn locations(id: u32, storage: State<Storage>) -> Option<Json<Location>> {
    storage
        .locations
        .get(&id)
        .map(|entity| Json(entity.clone()))
}

#[derive(FromForm)]
struct LocationAvgParams {}

#[get("/locations/<id>/avg")]
fn locations_avg_no_params(id: u32, storage: State<Storage>) -> Option<Json<HashMap<String, f32>>> {
    locations_avg(id, None, storage)
}

#[get("/locations/<id>/avg?<params>")]
fn locations_avg(
    id: u32,
    params: Option<LocationAvgParams>,
    storage: State<Storage>,
) -> Option<Json<HashMap<String, f32>>> {
    let all_visits = &storage.visits;

    Some(Json(HashMap::new()))
}

#[post("/locations/<id>")]
fn locations_update(id: u32) -> String {
    format!("location {} update", id)
}

#[post("/locations/new")]
fn locations_new() -> String {
    format!("locations new")
}

#[get("/visits/<id>")]
fn visits(id: u32, storage: State<Storage>) -> Option<Json<Visit>> {
    storage.visits.get(&id).map(|entity| Json(entity.clone()))
}

#[post("/visits/<id>")]
fn visits_update(id: u32) -> String {
    format!("visit {} update", id)
}

#[get("/visits/new")]
fn visits_new() -> String {
    format!("visit new")
}

fn get_env() -> String {
    match env::var("ENVIRONMENT") {
        Ok(val) => val,
        Err(_) => "dev".to_owned(),
    }
}

fn get_data_path(env: &str) -> Result<PathBuf, io::Error> {
    let cur_dir = env::current_dir()?;
    let mut data_dir = match &*env {
        "dev" => cur_dir.clone(),
        _ => PathBuf::from("tmp"),
    };
    data_dir.push("data");
    Ok(data_dir)
}

fn read_options(options_path: &Path) -> Result<String, io::Error> {
    let mut options_file = File::open(options_path).unwrap();
    let mut options_content = String::new();
    options_file.read_to_string(&mut options_content).unwrap();
    Ok(options_content)
}

struct Storage {
    users: HashMap<u32, User>,
    locations: HashMap<u32, Location>,
    visits: HashMap<u32, Visit>,
}

fn input_data(data_path: &Path) -> Result<Storage, io::Error> {
    let entity_name_templates = ["users_", "locations_", "visits_"];
    let mut all_users = HashMap::new();
    let mut all_locations = HashMap::new();
    let mut all_visits = HashMap::new();
    for template in &entity_name_templates {
        let mut index = 1;
        loop {
            let mut data_file_name = String::from(*template);
            data_file_name.push_str(&format!("{}", index));
            data_file_name.push_str(".json");
            let data_file_path = data_path.join(data_file_name);

            let maybe_data_file = File::open(data_file_path);
            let mut data_file = match maybe_data_file {
                Ok(f) => f,
                Err(_) => break,
            };
            let mut data = String::new();
            data_file.read_to_string(&mut data).unwrap();
            // TODO: Other entity types
            match &*template {
                &"users_" => {
                    let mut users: HashMap<String, Vec<User>> =
                        serde_json::from_str(&data).unwrap();
                    for v in users.remove("users").unwrap() {
                        all_users.insert(v.id, v);
                    }
                }
                &"locations_" => {
                    let mut locations: HashMap<String, Vec<Location>> =
                        serde_json::from_str(&data).unwrap();
                    for v in locations.remove("locations").unwrap() {
                        all_locations.insert(v.id, v);
                    }
                }
                &"visits_" => {
                    let mut visits: HashMap<String, Vec<Visit>> =
                        serde_json::from_str(&data).unwrap();
                    for v in visits.remove("visits").unwrap() {
                        all_visits.insert(v.id, v);
                    }
                }
                _ => unreachable!(),
            }
            index += 1;
        }
    }
    Ok(Storage {
        users: all_users,
        locations: all_locations,
        visits: all_visits,
    })
}

#[derive(Deserialize, Serialize, Debug, Clone)]
enum Gender {
    #[serde(rename = "m")]
    Male,
    #[serde(rename = "f")]
    Female,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct User {
    id: u32,
    email: String,      // [char; 100]
    first_name: String, // [char; 50]
    last_name: String,  // [char; 50]
    gender: Gender,
    birth_date: i32,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct Location {
    id: u32,
    place: String,
    country: String, // [char; 50]
    city: String,    // [char; 50]
    distance: u32,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct Visit {
    id: u32,
    location: u32,
    user: u32,
    visited_at: i32,
    mark: u8,
}

fn work() -> Result<(), Box<Error>> {
    let env = get_env();
    let data_path = get_data_path(&env).unwrap();
    let mut options_path = data_path.clone();
    options_path.push("options.txt");
    let _options = read_options(&options_path);
    let data = input_data(&data_path).unwrap();

    rocket::ignite()
        .manage(data)
        .mount(
            "/",
            routes![
                users,
                locations,
                visits,
                users_visits_no_params,
                users_visits,
                locations_avg,
                users_update,
                locations_update,
                visits_update,
            ],
        )
        .launch();
    Ok(())
}

fn main() {
    work().unwrap();
}
