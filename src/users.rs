use Gender;
use Options;
use QueryId;
use Storage;
use NewOrUpdateResponse;

use chrono::{Datelike, NaiveDateTime};
use rocket::State;
use rocket::http::Status;
use rocket::response::Failure;
use rocket_contrib::Json;

use std::collections::hash_map::Entry;

#[derive(Serialize)]
pub struct UserVisits {
    visits: Vec<VisitInfo>,
}

#[derive(Serialize, Deserialize, FromForm)]
pub struct UsersVisitsParams {
    #[form(field = "fromDate")] from_date: Option<i32>,
    #[form(field = "toDate")] to_date: Option<i32>,
    country: Option<String>,
    #[form(field = "toDistance")] to_distance: Option<u32>,
}

#[derive(Serialize, Deserialize)]
pub struct VisitInfo {
    mark: u8,
    visited_at: i32,
    place: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct User {
    pub id: u32,
    email: String,      // [char; 100]
    first_name: String, // [char; 50]
    last_name: String,  // [char; 50]
    pub gender: Gender,
    pub birth_date: i32,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct UserUpdate {
    #[serde(default)] email: String,      // [char; 100]
    #[serde(default)] first_name: String, // [char; 50]
    #[serde(default)] last_name: String,  // [char; 50]
    #[serde(default)] gender: Gender,
    #[serde(default)] birth_date: i32,
}

#[get("/users/<id>")]
fn users(id: u32, storage: State<Storage>) -> Option<Json<User>> {
    let users = &*storage.users.read().unwrap();
    users.get(&id).map(|entity| Json(entity.clone()))
}

#[get("/users/<id>/visits")]
fn users_visits_no_params(id: u32, storage: State<Storage>) -> Result<Json<UserVisits>, Failure> {
    users_visits(id, None, storage)
}

#[get("/users/<id>/visits?<params>")]
fn users_visits(
    id: u32,
    params: Option<UsersVisitsParams>,
    storage: State<Storage>,
) -> Result<Json<UserVisits>, Failure> {
    let all_users = &*storage.users.read().unwrap();
    {
        if let None = all_users.get(&id) {
            return Err(Failure(Status::NotFound));
        }
    }
    if let Some(ref params) = params {
        if params.country.is_none() && params.from_date.is_none() && params.to_date.is_none() &&
            params.to_distance.is_none()
        {
            return Err(Failure(Status::BadRequest));
        }
    }

    let all_visits = &*storage.visits.read().unwrap();
    let user_visits_ids = &storage.user_visits.read().unwrap();
    let maybe_this_user_visits_ids = user_visits_ids.get(&id);
    let user_visits = if let Some(this_user_visits_ids) = maybe_this_user_visits_ids {
        this_user_visits_ids.iter().map(|i| all_visits[&i].clone())
    } else {
        return Ok(Json(UserVisits { visits: vec![] }));
    };

    let mut result_visits = if let Some(params) = params {
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
            let locations = &storage.locations.read().unwrap();
            let reference_country = &locations.get(&v.location).unwrap().country;

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
                let locations = &storage.locations.read().unwrap();
                let reference_distance = locations.get(&v.location).unwrap().distance;

                if to_distance > reference_distance {
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
            let location = &locations.read().unwrap()[&v.location];
            let place = location.place.clone();
            VisitInfo {
                mark: v.mark,
                place: place,
                visited_at: v.visited_at,
            }
        })
        .collect();

    let response = UserVisits {
        visits: result_visits,
    };
    Ok(Json(response))
}

#[post("/users/<id>?<query_id>", data = "<user>")]
fn users_update(
    id: u32,
    user: Json<UserUpdate>,
    query_id: QueryId,
    storage: State<Storage>,
    options: State<Options>,
) -> Option<Json<NewOrUpdateResponse>> {
    let _query_id = query_id;
    let user_update = user.0;

    let users = &mut *storage.users.write().unwrap();
    let user_entry = users.entry(id);
    match user_entry {
        Entry::Occupied(mut e) => {
            if user_update.email != "" {
                e.get_mut().email = user_update.email;
            }
            if user_update.birth_date != 0 {
                e.get_mut().birth_date = user_update.birth_date;

                let ages = &mut *storage.ages.write().unwrap();
                ages.insert(
                    id,
                    calculate_age_from_timestamp(user_update.birth_date, options.now),
                );
            }
            if user_update.first_name != "" {
                e.get_mut().first_name = user_update.first_name;
            }
            if user_update.last_name != "" {
                e.get_mut().last_name = user_update.last_name;
            }
            if user_update.gender != Gender::Unknown {
                e.get_mut().gender = user_update.gender;
            }
        }
        Entry::Vacant(_) => return None,
    }
    Some(Json(NewOrUpdateResponse))
}

#[post("/users/new", data = "<user>")]
fn users_new(
    user: Json<User>,
    storage: State<Storage>,
    options: State<Options>,
) -> Option<Json<NewOrUpdateResponse>> {
    let user = user.0;
    let id = user.id;

    let users = &mut *storage.users.write().unwrap();
    let ages = &mut *storage.ages.write().unwrap();
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
            ages.insert(
                id,
                calculate_age_from_timestamp(user.birth_date, options.now),
            );
        }
    }
    Some(Json(NewOrUpdateResponse))
}

pub fn calculate_age_from_timestamp(birth_date_timestamp: i32, now_timestamp: i32) -> i32 {
    let birth_date = NaiveDateTime::from_timestamp(birth_date_timestamp as i64, 0);

    let now = NaiveDateTime::from_timestamp(now_timestamp as i64, 0);

    let birth_date_year = birth_date.year();
    let now_year = now.year();

    let year_diff = now_year - birth_date_year;

    let birth_date_in_year_month = birth_date.month();
    let now_in_year_month = now.month();
    let birth_date_in_year_day = birth_date.day();
    let now_in_year_day = now.day();

    let did_full_year_pass = if now_in_year_month > birth_date_in_year_month ||
        (now_in_year_month == birth_date_in_year_month && now_in_year_day >= birth_date_in_year_day)
    {
        true
    } else {
        false
    };

    let year_correction = if !did_full_year_pass { -1 } else { 0 };

    let age = year_diff + year_correction;
    age
}
