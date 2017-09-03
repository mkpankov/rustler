#![feature(plugin, custom_derive)]
#![plugin(rocket_codegen)]

extern crate chrono;
extern crate rocket;
extern crate rocket_contrib;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate zip;

mod users;

#[cfg(test)]
mod tests;

use users::User;

use rocket::State;
use rocket::http::{RawStr, Status};
use rocket::request::FromFormValue;
use rocket::response::Failure;
use rocket_contrib::Json;
use serde::{Serialize, Serializer};
use serde::ser::SerializeMap;

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::env;
use std::error::Error;
use std::io::{self, BufReader, Read};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

#[derive(FromForm)]
struct QueryId {
    #[form(field = "query_id")] _query_id: u32,
}

struct NewOrUpdateResponse;

impl Serialize for NewOrUpdateResponse {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let map = serializer.serialize_map(Some(0))?;
        map.end()
    }
}

#[get("/locations/<id>")]
fn locations(id: u32, storage: State<Storage>) -> Option<Json<Location>> {
    let locations = &*storage.locations.read().unwrap();
    locations.get(&id).map(|entity| Json(entity.clone()))
}

#[derive(FromForm, Debug)]
struct LocationAvgParams {
    #[form(field = "fromDate")] from_date: Option<i32>,
    #[form(field = "toDate")] to_date: Option<i32>,
    #[form(field = "fromAge")] from_age: Option<i32>,
    #[form(field = "toAge")] to_age: Option<i32>,
    gender: Option<Gender>,
}

impl<'v> FromFormValue<'v> for Gender {
    type Error = &'v RawStr;

    fn from_form_value(form_value: &'v RawStr) -> Result<Gender, &'v RawStr> {
        match form_value.as_str() {
            "m" => Ok(Gender::Male),
            "f" => Ok(Gender::Female),
            _ => Err(form_value),
        }
    }
}

#[get("/locations/<id>/avg")]
fn locations_avg_no_params(
    id: u32,
    storage: State<Storage>,
    options: State<Options>,
) -> Result<Json<LocationAvg>, Failure> {
    locations_avg(id, None, storage, options)
}

#[get("/locations/<id>/avg?<params>")]
fn locations_avg(
    id: u32,
    params: Option<LocationAvgParams>,
    storage: State<Storage>,
    options: State<Options>,
) -> Result<Json<LocationAvg>, Failure> {
    let all_locations = &storage.locations.read().unwrap();
    {
        if let None = all_locations.get(&id) {
            return Err(Failure(Status::NotFound));
        }
    }

    if let Some(ref params) = params {
        if let Some(ref gender) = params.gender {
            match *gender {
                Gender::Unknown => return Err(Failure(Status::BadRequest)),
                _ => {}
            };
        }

        if params.from_age.is_none() && params.from_date.is_none() && params.gender.is_none() &&
            params.to_age.is_none() && params.to_date.is_none()
        {
            return Err(Failure(Status::BadRequest));
        }
    }

    let all_visits = &storage.visits.read().unwrap();
    let location_visits_ids = &storage.location_visits.read().unwrap();
    let maybe_this_location_visits_ids = location_visits_ids.get(&id);
    let location_visits = if let Some(this_location_visits_ids) = maybe_this_location_visits_ids {
        this_location_visits_ids
            .iter()
            .map(|i| all_visits[&i].clone())
    } else {
        return Ok(Json(LocationAvg { avg: 0. }));
    };

    let result_visits: Vec<_> = if let Some(params) = params {
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
            let users = &storage.users.read().unwrap();
            let user = users.get(&v.user).unwrap();

            let birth_date_timestamp = user.birth_date;
            let age = users::calculate_age_from_timestamp(birth_date_timestamp, options.now);

            if from_age <= age {
                true
            } else {
                false
            }
        } else {
            true
        });

        let to_age_visits = from_age_visits.filter(|v| if let Some(to_age) = params.to_age {
            let users = &storage.users.read().unwrap();
            let user = users.get(&v.user).unwrap();

            let birth_date_timestamp = user.birth_date;
            let age = users::calculate_age_from_timestamp(birth_date_timestamp, options.now);

            if to_age > age {
                true
            } else {
                false
            }
        } else {
            true
        });

        let final_visits = to_age_visits.filter(|v| if let Some(ref gender) = params.gender {
            let users = &storage.users.read().unwrap();
            let user = users.get(&v.user).unwrap();
            let reference_gender = &user.gender;

            if gender == reference_gender {
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

    Ok(Json(LocationAvg {
        avg: avg_mark_rounded,
    }))
}

#[derive(Serialize)]
struct LocationAvg {
    avg: f64,
}

#[post("/locations/<id>?<query_id>", data = "<location>")]
fn locations_update(
    id: u32,
    location: Json<LocationUpdate>,
    query_id: QueryId,
    storage: State<Storage>,
) -> Option<Json<NewOrUpdateResponse>> {
    let _query_id = query_id;
    let location_update = location.0;

    let locations = &mut storage.locations.write().unwrap();
    let location_entry = locations.entry(id);
    match location_entry {
        Entry::Occupied(mut e) => {
            if location_update.city != "" {
                e.get_mut().city = location_update.city;
            }
            if location_update.country != "" {
                e.get_mut().country = location_update.country;
            }
            if location_update.distance != 0 {
                e.get_mut().distance = location_update.distance;
            }
            if location_update.place != "" {
                e.get_mut().place = location_update.place;
            }
        }
        Entry::Vacant(_) => return None,
    }
    Some(Json(NewOrUpdateResponse))
}

#[post("/locations/new", data = "<location>")]
fn locations_new(
    location: Json<Location>,
    storage: State<Storage>,
) -> Option<Json<NewOrUpdateResponse>> {
    let location = location.0;
    let id = location.id;

    let locations = &mut storage.locations.write().unwrap();
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
    Some(Json(NewOrUpdateResponse))
}

#[get("/visits/<id>")]
fn visits(id: u32, storage: State<Storage>) -> Option<Json<Visit>> {
    storage
        .visits
        .read()
        .unwrap()
        .get(&id)
        .map(|entity| Json(entity.clone()))
}

#[post("/visits/<id>?<query_id>", data = "<visit>")]
fn visits_update(
    id: u32,
    visit: Json<VisitUpdate>,
    query_id: QueryId,
    storage: State<Storage>,
) -> Option<Json<NewOrUpdateResponse>> {
    let _query_id = query_id;
    let visit_update = visit.0;

    let visits = &mut storage.visits.write().unwrap();
    let visit_entry = visits.entry(id);
    match visit_entry {
        Entry::Occupied(mut e) => {
            if visit_update.location != 0 {
                let location_visits_ids = &mut storage.location_visits.write().unwrap();
                let new_visit_location = visit_update.location;
                let old_visit_location = e.get().location;

                let old_location_visits_ids = location_visits_ids[&old_visit_location]
                    .iter()
                    .map(|visit_id| *visit_id)
                    .filter(|visit_id| *visit_id != id)
                    .collect();

                {
                    let new_location_visits_ids_entry =
                        location_visits_ids.entry(new_visit_location);
                    match new_location_visits_ids_entry {
                        Entry::Occupied(mut e) => {
                            e.get_mut().push(id);
                        }
                        Entry::Vacant(e) => {
                            e.insert(vec![id]);
                        }
                    }
                }

                let new_location_visits_ids = location_visits_ids[&new_visit_location].clone();

                location_visits_ids.insert(old_visit_location, old_location_visits_ids);
                location_visits_ids.insert(new_visit_location, new_location_visits_ids);
                e.get_mut().location = visit_update.location;
            };
            if visit_update.mark != 0 {
                e.get_mut().mark = visit_update.mark;
            }
            if visit_update.user != 0 {
                let user_visits_ids = &mut storage.user_visits.write().unwrap();
                let new_visit_user = visit_update.user;
                let old_visit_user = e.get().user;

                let old_user_visits_ids = user_visits_ids[&old_visit_user]
                    .iter()
                    .map(|visit_id| *visit_id)
                    .filter(|visit_id| *visit_id != id)
                    .collect();

                {
                    let new_user_visits_ids_entry = user_visits_ids.entry(new_visit_user);
                    match new_user_visits_ids_entry {
                        Entry::Occupied(mut e) => {
                            e.get_mut().push(id);
                        }
                        Entry::Vacant(e) => {
                            e.insert(vec![id]);
                        }
                    }
                }

                let new_user_visits_ids = user_visits_ids[&new_visit_user].clone();

                user_visits_ids.insert(old_visit_user, old_user_visits_ids);
                user_visits_ids.insert(new_visit_user, new_user_visits_ids);

                e.get_mut().user = visit_update.user;
            }
            if visit_update.visited_at != 0 {
                e.get_mut().visited_at = visit_update.visited_at;
            }
        }
        Entry::Vacant(_) => return None,
    }
    Some(Json(NewOrUpdateResponse))
}

#[post("/visits/new", data = "<visit>")]
fn visits_new(visit: Json<Visit>, storage: State<Storage>) -> Option<Json<NewOrUpdateResponse>> {
    let visit = visit.0;
    let id = visit.id;

    let visits = &mut storage.visits.write().unwrap();
    let visit_entry = visits.entry(id);
    match visit_entry {
        Entry::Occupied(_) => return None,
        Entry::Vacant(e) => {
            let location_visits_ids = &mut storage.location_visits.write().unwrap();
            let new_visit_location = visit.location;

            {
                let new_location_visits_ids_entry = location_visits_ids.entry(new_visit_location);
                match new_location_visits_ids_entry {
                    Entry::Occupied(mut e) => {
                        e.get_mut().push(id);
                    }
                    Entry::Vacant(e) => {
                        e.insert(vec![id]);
                    }
                }
            }
            let new_location_visits_ids = location_visits_ids[&new_visit_location].clone();

            location_visits_ids.insert(new_visit_location, new_location_visits_ids);

            let user_visits_ids = &mut storage.user_visits.write().unwrap();
            let new_visit_user = visit.user;

            {
                let new_user_visits_ids_entry = user_visits_ids.entry(new_visit_user);
                match new_user_visits_ids_entry {
                    Entry::Occupied(mut e) => {
                        e.get_mut().push(id);
                    }
                    Entry::Vacant(e) => {
                        e.insert(vec![id]);
                    }
                }
            }
            let new_user_visits_ids = user_visits_ids[&new_visit_user].clone();

            user_visits_ids.insert(new_visit_user, new_user_visits_ids);

            e.insert(Visit {
                id: id,
                location: visit.location,
                mark: visit.mark,
                user: visit.user,
                visited_at: visit.visited_at,
            });
        }
    }
    Some(Json(NewOrUpdateResponse))
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
    users: RwLock<HashMap<u32, User>>,
    locations: RwLock<HashMap<u32, Location>>,
    visits: RwLock<HashMap<u32, Visit>>,
    ages: RwLock<HashMap<u32, i32>>,
    location_visits: RwLock<HashMap<u32, Vec<u32>>>,
    user_visits: RwLock<HashMap<u32, Vec<u32>>>,
}

fn read_entities_from_file<T>(
    data_file: &mut T,
    template: &str,
    all_users: &mut HashMap<u32, User>,
    all_locations: &mut HashMap<u32, Location>,
    all_visits: &mut HashMap<u32, Visit>,
    ages: &mut HashMap<u32, i32>,
    location_visits: &mut HashMap<u32, Vec<u32>>,
    user_visits: &mut HashMap<u32, Vec<u32>>,
    options: &Options,
) where
    T: Read,
{
    let mut data = String::new();
    data_file.read_to_string(&mut data).unwrap();

    match template {
        "users_" => {
            let mut users: HashMap<String, Vec<User>> = serde_json::from_str(&data).unwrap();
            for v in users.remove("users").unwrap() {
                ages.insert(
                    v.id,
                    users::calculate_age_from_timestamp(v.birth_date, options.now),
                );
                all_users.insert(v.id, v);
            }
        }
        "locations_" => {
            let mut locations: HashMap<String, Vec<Location>> =
                serde_json::from_str(&data).unwrap();
            for v in locations.remove("locations").unwrap() {
                all_locations.insert(v.id, v);
            }
        }
        "visits_" => {
            let mut visits: HashMap<String, Vec<Visit>> = serde_json::from_str(&data).unwrap();
            for visit in visits.remove("visits").unwrap() {
                let location_visits_entry = location_visits.entry(visit.location);
                match location_visits_entry {
                    Entry::Occupied(mut e) => {
                        e.get_mut().push(visit.id);
                    }
                    Entry::Vacant(e) => {
                        e.insert(vec![visit.id]);
                    }
                }

                let user_visits_entry = user_visits.entry(visit.user);
                match user_visits_entry {
                    Entry::Occupied(mut e) => {
                        e.get_mut().push(visit.id);
                    }
                    Entry::Vacant(e) => {
                        e.insert(vec![visit.id]);
                    }
                }

                all_visits.insert(visit.id, visit);
            }
        }
        _ => unreachable!(),
    }
}

fn input_data(data_path: &Path, options: &Options) -> Result<Storage, io::Error> {
    let entity_name_templates = ["users_", "locations_", "visits_"];
    let mut all_users = HashMap::new();
    let mut all_locations = HashMap::new();
    let mut all_visits = HashMap::new();
    let mut ages = HashMap::new();
    let mut location_visits = HashMap::new();
    let mut user_visits = HashMap::new();

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
            read_entities_from_file(
                &mut data_file,
                template,
                &mut all_users,
                &mut all_locations,
                &mut all_visits,
                &mut ages,
                &mut location_visits,
                &mut user_visits,
                &options,
            );
            index += 1;
        }
    }

    Ok(Storage {
        users: RwLock::new(all_users),
        locations: RwLock::new(all_locations),
        visits: RwLock::new(all_visits),
        ages: RwLock::new(ages),
        location_visits: RwLock::new(location_visits),
        user_visits: RwLock::new(user_visits),
    })
}

fn input_data_prod(data_dir_path: &Path, options: &Options) -> Result<Storage, io::Error> {
    let entity_name_templates = ["users_", "locations_", "visits_"];
    let mut all_users = HashMap::new();
    let mut all_locations = HashMap::new();
    let mut all_visits = HashMap::new();
    let mut ages = HashMap::new();
    let mut location_visits = HashMap::new();
    let mut user_visits = HashMap::new();

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
                Err(_) => break,
            };
            read_entities_from_file(
                &mut data_file,
                template,
                &mut all_users,
                &mut all_locations,
                &mut all_visits,
                &mut ages,
                &mut location_visits,
                &mut user_visits,
                &options,
            );
            index += 1;
        }
    }

    Ok(Storage {
        users: RwLock::new(all_users),
        locations: RwLock::new(all_locations),
        visits: RwLock::new(all_visits),
        ages: RwLock::new(ages),
        location_visits: RwLock::new(location_visits),
        user_visits: RwLock::new(user_visits),
    })
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub enum Gender {
    Unknown,
    #[serde(rename = "m")] Male,
    #[serde(rename = "f")] Female,
}

impl Default for Gender {
    fn default() -> Self {
        Gender::Unknown
    }
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
    #[serde(default)] place: String,
    #[serde(default)] country: String, // [char; 50]
    #[serde(default)] city: String,    // [char; 50]
    #[serde(default)] distance: u32,
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
    #[serde(default)] location: u32,
    #[serde(default)] user: u32,
    #[serde(default)] visited_at: i32,
    #[serde(default)] mark: u8,
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
        "prod" => input_data_prod(&data_dir_path, &options).unwrap(),
        "dev" => input_data(&data_dir_path, &options).unwrap(),
        _ => unreachable!(),
    };

    rocket::ignite()
        .manage(data)
        .manage(options)
        .mount(
            "/",
            routes![
                users::users,
                locations,
                visits,
                users::users_visits_no_params,
                users::users_visits,
                locations_avg_no_params,
                locations_avg,
                users::users_update,
                locations_update,
                visits_update,
                users::users_new,
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
