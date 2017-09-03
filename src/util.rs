use serde::{Serialize, Serializer};
use serde::ser::SerializeMap;

#[derive(FromForm)]
pub struct QueryId {
    #[form(field = "query_id")] _query_id: u32,
}

pub struct NewOrUpdateResponse;

impl Serialize for NewOrUpdateResponse {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let map = serializer.serialize_map(Some(0))?;
        map.end()
    }
}
