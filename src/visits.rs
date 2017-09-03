use QueryId;
use Storage;
use NewOrUpdateResponse;

use rocket::State;
use rocket_contrib::Json;

use std::collections::hash_map::Entry;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Visit {
    pub id: u32,
    pub location: u32,
    pub user: u32,
    pub visited_at: i32,
    pub mark: u8,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct VisitUpdate {
    #[serde(default)] location: u32,
    #[serde(default)] user: u32,
    #[serde(default)] visited_at: i32,
    #[serde(default)] mark: u8,
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
