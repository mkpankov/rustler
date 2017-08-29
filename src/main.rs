#![feature(plugin, custom_derive)]
#![plugin(rocket_codegen)]

extern crate chrono;
extern crate libc;
extern crate rocket;
extern crate rocket_contrib;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate zip;

use chrono::{DateTime, NaiveDateTime, Utc};
use rocket::State;
use rocket::http::Status;
use rocket::response::Failure;
use rocket_contrib::Json;

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::env;
use std::error::Error;
use std::io::{self, BufReader, Read};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

#[cfg(test)]
mod tests;

#[get("/users/<id>")]
fn users(id: u32, storage: State<RwLock<Storage>>) -> Option<Json<User>> {
    let storage = &*storage.read().unwrap();
    storage.users.get(&id).map(|entity| Json(entity.clone()))
}

#[derive(Serialize, Deserialize, FromForm)]
struct UsersVisitsParams {
    #[form(field = "fromDate")]
    from_date: Option<i32>,
    #[form(field = "toDate")]
    to_date: Option<i32>,
    country: Option<String>,
    #[form(field = "toDistance")]
    to_distance: Option<u32>,
}

#[derive(Serialize, Deserialize)]
struct VisitInfo {
    mark: u8,
    visited_at: i32,
    place: String,
}

#[get("/users/<id>/visits")]
fn users_visits_no_params(
    id: u32,
    storage: State<RwLock<Storage>>,
) -> Result<Json<HashMap<String, Vec<VisitInfo>>>, Failure> {
    users_visits(id, None, storage)
}

#[get("/users/<id>/visits?<params>")]
fn users_visits(
    id: u32,
    params: Option<UsersVisitsParams>,
    storage: State<RwLock<Storage>>,
) -> Result<Json<HashMap<String, Vec<VisitInfo>>>, Failure> {
    let storage = &*storage.read().unwrap();
    let all_users = &storage.users;
    {
        if let None = all_users.get(&id) {
            return Err(Failure(Status::NotFound));
        }
    }
    let all_visits = &storage.visits;
    let user_visits = all_visits
        .iter()
        .map(|(_, v)| v)
        .cloned()
        .filter(|v| v.user == id);

    let mut result_visits = if let Some(params) = params {
        if params.country.is_none() && params.from_date.is_none() && params.to_date.is_none() &&
            params.to_distance.is_none()
        {
            return Err(Failure(Status::BadRequest));
        }

        let from_date_visits = user_visits.filter(|v| if let Some(from_date) = params.from_date {
            if from_date < v.visited_at {
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

                if to_distance > *reference_distance {
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

    result_visits.sort_by(|v1, v2| v1.visited_at.cmp(&v2.visited_at));
    let result_visits = result_visits
        .iter()
        .map(|v| {
            let locations = &storage.locations;
            let location = &locations[&v.location];
            let place = location.place.clone();
            VisitInfo {
                mark: v.mark,
                place: place,
                visited_at: v.visited_at,
            }
        })
        .collect();

    let mut response = HashMap::new();
    response.insert("visits".to_owned(), result_visits);
    Ok(Json(response))
}

#[derive(FromForm)]
struct QueryId {
    #[form(field = "query_id")]
    _query_id: u32,
}

#[post("/users/<id>?<query_id>", data = "<user>")]
fn users_update(
    id: u32,
    user: Json<UserUpdate>,
    query_id: QueryId,
    storage: State<RwLock<Storage>>,
) -> Option<Json<HashMap<(), ()>>> {
    let storage = &mut *storage.write().unwrap();
    let user_update = user.0;

    let users = &mut storage.users;
    let user_entry = users.entry(id);
    match user_entry {
        Entry::Occupied(mut e) => {
            if let Some(email) = user_update.email {
                e.get_mut().email = email;
            }
            if let Some(birth_date) = user_update.birth_date {
                e.get_mut().birth_date = birth_date;
            }
            if let Some(first_name) = user_update.first_name {
                e.get_mut().first_name = first_name;
            }
            if let Some(last_name) = user_update.last_name {
                e.get_mut().last_name = last_name;
            }
            if let Some(gender) = user_update.gender {
                e.get_mut().gender = gender;
            }
        }
        Entry::Vacant(_) => return None,
    }
    Some(Json(HashMap::new()))
}

#[post("/users/new", data = "<user>")]
fn users_new(user: Json<User>, storage: State<RwLock<Storage>>) -> Option<Json<HashMap<(), ()>>> {
    let storage = &mut *storage.write().unwrap();
    let user = user.0;
    let id = user.id;

    let users = &mut storage.users;
    let user_entry = users.entry(id);
    match user_entry {
        Entry::Occupied(_) => return None,
        Entry::Vacant(e) => {
            e.insert(User {
                id: id,
                email: user.email,
                birth_date: user.birth_date,
                first_name: user.first_name,
                last_name: user.last_name,
                gender: user.gender,
            });
        }
    }
    Some(Json(HashMap::new()))
}

#[get("/locations/<id>")]
fn locations(id: u32, storage: State<RwLock<Storage>>) -> Option<Json<Location>> {
    let storage = &*storage.read().unwrap();
    storage
        .locations
        .get(&id)
        .map(|entity| Json(entity.clone()))
}

#[derive(FromForm)]
struct LocationAvgParams {
    #[form(field = "fromDate")]
    from_date: Option<i32>,
    #[form(field = "toDate")]
    to_date: Option<i32>,
    #[form(field = "fromAge")]
    from_age: Option<i32>,
    #[form(field = "toAge")]
    to_age: Option<i32>,
    gender: Option<String>,
}

#[get("/locations/<id>/avg")]
fn locations_avg_no_params(
    id: u32,
    storage: State<RwLock<Storage>>,
    options: State<Options>,
) -> Result<Json<HashMap<String, f64>>, Failure> {
    locations_avg(id, None, storage, options)
}

#[get("/locations/<id>/avg?<params>")]
fn locations_avg(
    id: u32,
    params: Option<LocationAvgParams>,
    storage: State<RwLock<Storage>>,
    options: State<Options>,
) -> Result<Json<HashMap<String, f64>>, Failure> {
    let storage = &*storage.read().unwrap();
    let all_locations = &storage.locations;
    {
        if let None = all_locations.get(&id) {
            return Err(Failure(Status::NotFound));
        }
    }
    let all_visits = &storage.visits;
    let location_visits = all_visits.values().cloned().filter(|v| v.location == id);

    let result_visits: Vec<_> = if let Some(params) = params {
        if params.from_age.is_none() && params.from_date.is_none() && params.gender.is_none() &&
            params.to_age.is_none() &&
            params.to_date.is_none()
        {
            return Err(Failure(Status::BadRequest));
        }

        let from_date_visits =
            location_visits.filter(|v| if let Some(from_date) = params.from_date {
                if from_date < v.visited_at {
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

        let from_age_visits = to_date_visits.filter(|v| if let Some(from_age) = params.from_age {
            let user = &storage.users.get(&v.user).unwrap();

            let birth_date_timestamp = user.birth_date;
            let birth_date = NaiveDateTime::from_timestamp(birth_date_timestamp as i64, 0);
            let birth_date = DateTime::<Utc>::from_utc(birth_date, Utc);

            let now = NaiveDateTime::from_timestamp(options.now as i64, 0);
            let now = DateTime::<Utc>::from_utc(now, Utc);

            let age = now.signed_duration_since(birth_date);
            let age_years = age.num_days() as f64 / 365.2425;

            if from_age < age_years.round() as i32 {
                true
            } else {
                false
            }
        } else {
            true
        });

        let to_age_visits = from_age_visits.filter(|v| if let Some(to_age) = params.to_age {
            let user = &storage.users.get(&v.user).unwrap();

            let birth_date_timestamp = user.birth_date;
            let birth_date = NaiveDateTime::from_timestamp(birth_date_timestamp as i64, 0);
            let birth_date = DateTime::<Utc>::from_utc(birth_date, Utc);

            let now = NaiveDateTime::from_timestamp(options.now as i64, 0);
            let now = DateTime::<Utc>::from_utc(now, Utc);

            let age = now.signed_duration_since(birth_date);
            let age_years = age.num_days() as f64 / 365.2425;

            if to_age > age_years.round() as i32 {
                true
            } else {
                false
            }
        } else {
            true
        });

        if let Some(ref gender) = params.gender {
            match gender.as_ref() {
                "m" | "f" => {}
                _ => return Err(Failure(Status::BadRequest)),
            };
        }

        let final_visits = to_age_visits.filter(|v| if let Some(ref gender) = params.gender {
            let user = &storage.users.get(&v.user).unwrap();
            let reference_gender = &user.gender;

            let parsed_gender = match gender.as_ref() {
                "m" => Gender::Male,
                "f" => Gender::Female,
                _ => unreachable!(),
            };
            if parsed_gender == *reference_gender {
                true
            } else {
                false
            }
        } else {
            true
        });

        final_visits.collect()
    } else {
        location_visits.collect()
    };

    let marks = result_visits.iter().map(|v| v.mark);
    let mut sum: usize = 0;
    for v in marks {
        sum += v as usize;
    }
    let avg_mark: f64 = if sum > 0 {
        sum as f64 / result_visits.len() as f64
    } else {
        0.0
    };
    let avg_mark_rounded = format!("{:.5}", avg_mark);
    let avg_mark_rounded: f64 = avg_mark_rounded.parse().unwrap();

    let mut result = HashMap::new();
    result.insert("avg".to_owned(), avg_mark_rounded);
    Ok(Json(result))
}

#[post("/locations/<id>", data = "<location>")]
fn locations_update(
    id: u32,
    location: Json<LocationUpdate>,
    storage: State<RwLock<Storage>>,
) -> Option<Json<HashMap<(), ()>>> {
    let storage = &mut *storage.write().unwrap();
    let location_update = location.0;

    let locations = &mut storage.locations;
    let location_entry = locations.entry(id);
    match location_entry {
        Entry::Occupied(mut e) => {
            if let Some(city) = location_update.city {
                e.get_mut().city = city;
            }
            if let Some(country) = location_update.country {
                e.get_mut().country = country;
            }
            if let Some(distance) = location_update.distance {
                e.get_mut().distance = distance;
            }
            if let Some(place) = location_update.place {
                e.get_mut().place = place;
            }
        }
        Entry::Vacant(_) => return None,
    }
    Some(Json(HashMap::new()))
}

#[post("/locations/new", data = "<location>")]
fn locations_new(
    location: Json<Location>,
    storage: State<RwLock<Storage>>,
) -> Option<Json<HashMap<(), ()>>> {
    let storage = &mut *storage.write().unwrap();
    let location = location.0;
    let id = location.id;

    let locations = &mut storage.locations;
    let location_entry = locations.entry(id);
    match location_entry {
        Entry::Occupied(_) => return None,
        Entry::Vacant(e) => {
            e.insert(Location {
                id: id,
                city: location.city,
                country: location.country,
                distance: location.distance,
                place: location.place,
            });
        }
    }
    Some(Json(HashMap::new()))
}

#[get("/visits/<id>")]
fn visits(id: u32, storage: State<RwLock<Storage>>) -> Option<Json<Visit>> {
    let storage = &*storage.read().unwrap();
    storage.visits.get(&id).map(|entity| Json(entity.clone()))
}

#[post("/visits/<id>", data = "<visit>")]
fn visits_update(
    id: u32,
    visit: Json<VisitUpdate>,
    storage: State<RwLock<Storage>>,
) -> Option<Json<HashMap<(), ()>>> {
    let storage = &mut *storage.write().unwrap();
    let visit_update = visit.0;

    let visits = &mut storage.visits;
    let visit_entry = visits.entry(id);
    match visit_entry {
        Entry::Occupied(mut e) => {
            if let Some(location) = visit_update.location {
                e.get_mut().location = location;
            };
            if let Some(mark) = visit_update.mark {
                e.get_mut().mark = mark;
            }
            if let Some(user) = visit_update.user {
                e.get_mut().user = user;
            }
            if let Some(visited_at) = visit_update.visited_at {
                e.get_mut().visited_at = visited_at;
            }
        }
        Entry::Vacant(_) => return None,
    }
    Some(Json(HashMap::new()))
}

#[post("/visits/new", data = "<visit>")]
fn visits_new(
    visit: Json<Visit>,
    storage: State<RwLock<Storage>>,
) -> Option<Json<HashMap<(), ()>>> {
    let storage = &mut *storage.write().unwrap();
    let visit = visit.0;
    let id = visit.id;

    let visits = &mut storage.visits;
    let visit_entry = visits.entry(id);
    match visit_entry {
        Entry::Occupied(_) => return None,
        Entry::Vacant(e) => {
            e.insert(Visit {
                id: id,
                location: visit.location,
                mark: visit.mark,
                user: visit.user,
                visited_at: visit.visited_at,
            });
        }
    }
    Some(Json(HashMap::new()))
}

fn get_env() -> String {
    match env::var("ENVIRONMENT") {
        Ok(val) => val,
        Err(_) => "dev".to_owned(),
    }
}

fn get_data_dir_path(env: &str) -> Result<PathBuf, io::Error> {
    let data_dir = match &*env {
        "dev" => {
            let mut cur_dir = env::current_dir()?;
            cur_dir.push("data");
            cur_dir
        }
        "prod" => PathBuf::from("/tmp/data"),
        _ => unreachable!(),
    };
    Ok(data_dir)
}

fn read_options(options_path: &Path) -> Result<Options, io::Error> {
    let mut options_file = File::open(options_path).unwrap();
    let mut options_content = String::new();
    options_file.read_to_string(&mut options_content).unwrap();
    let options_content_lines: Vec<_> = options_content.split('\n').collect();
    let timestamp_line = options_content_lines[0];
    let timestamp: i32 = timestamp_line.parse().unwrap();
    println!("now: {}", timestamp);
    let mode_line = options_content_lines[1];
    let mode = match mode_line {
        "0" => Mode::Test,
        "1" => Mode::Rating,
        _ => unreachable!(),
    };
    Ok(Options {
        now: timestamp,
        mode: mode,
    })
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

            println!("Reading data file: {:?}", data_file_path);
            let maybe_data_file = File::open(data_file_path);
            let mut data_file = match maybe_data_file {
                Ok(f) => f,
                Err(_) => break,
            };
            let mut data = String::new();
            data_file.read_to_string(&mut data).unwrap();

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

fn input_data_prod(data_dir_path: &Path) -> Result<Storage, io::Error> {
    let entity_name_templates = ["users_", "locations_", "visits_"];
    let mut all_users = HashMap::new();
    let mut all_locations = HashMap::new();
    let mut all_visits = HashMap::new();

    let data_file_path = data_dir_path.join("data.zip");
    let file = File::open(data_file_path).unwrap();
    let reader = BufReader::new(file);

    let mut zip = zip::ZipArchive::new(reader).unwrap();

    for template in &entity_name_templates {
        let mut index = 1;
        loop {
            let mut data_file_name = String::from(*template);
            data_file_name.push_str(&format!("{}", index));
            data_file_name.push_str(".json");

            println!("Reading data file: {:?}", data_file_name);
            let maybe_data_file = zip.by_name(data_file_name.as_str());
            let mut data_file = match maybe_data_file {
                Ok(f) => f,
                Err(e) => {
                    println!("error: {}", e);
                    break;
                }
            };
            let mut data = String::new();
            data_file.read_to_string(&mut data).unwrap();

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

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
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
struct UserUpdate {
    email: Option<String>,      // [char; 100]
    first_name: Option<String>, // [char; 50]
    last_name: Option<String>,  // [char; 50]
    gender: Option<Gender>,
    birth_date: Option<i32>,
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
struct LocationUpdate {
    place: Option<String>,
    country: Option<String>, // [char; 50]
    city: Option<String>,    // [char; 50]
    distance: Option<u32>,
    _query_id: Option<u32>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct Visit {
    id: u32,
    location: u32,
    user: u32,
    visited_at: i32,
    mark: u8,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct VisitUpdate {
    location: Option<u32>,
    user: Option<u32>,
    visited_at: Option<i32>,
    mark: Option<u8>,
}

#[derive(Debug)]
enum Mode {
    Test,
    Rating,
}

#[derive(Debug)]
struct Options {
    now: i32,
    mode: Mode,
}

fn work() -> Result<(), Box<Error>> {
    let env = get_env();
    println!("env: {:?}", env);
    let data_dir_path = get_data_dir_path(&env).unwrap();
    println!("data_dir_path: {:?}", data_dir_path);

    let mut options_path = data_dir_path.clone();
    options_path.push("options.txt");
    let options = read_options(&options_path).unwrap();
    let data = match &*env {
        "prod" => input_data_prod(&data_dir_path).unwrap(),
        "dev" => input_data(&data_dir_path).unwrap(),
        _ => unreachable!(),
    };
    let data_locked = RwLock::new(data);

    rocket::ignite()
        .manage(data_locked)
        .manage(options)
        .mount(
            "/",
            routes![
                users,
                locations,
                visits,
                users_visits_no_params,
                users_visits,
                locations_avg_no_params,
                locations_avg,
                users_update,
                locations_update,
                visits_update,
                users_new,
                locations_new,
                visits_new,
            ],
        )
        .launch();
    Ok(())
}

fn main() {
    work().unwrap();
}
