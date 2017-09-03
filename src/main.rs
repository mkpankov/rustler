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
mod locations;
mod visits;

#[cfg(test)]
mod tests;

use users::User;
use locations::Location;
use visits::Visit;

use rocket::http::RawStr;
use rocket::request::FromFormValue;
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
                locations::locations,
                visits::visits,
                users::users_visits_no_params,
                users::users_visits,
                locations::locations_avg_no_params,
                locations::locations_avg,
                users::users_update,
                locations::locations_update,
                visits::visits_update,
                users::users_new,
                locations::locations_new,
                visits::visits_new,
            ],
        )
        .launch();
    Ok(())
}

fn main() {
    work().unwrap();
}
