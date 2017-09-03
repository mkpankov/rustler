use rocket::http::RawStr;
use rocket::request::FromFormValue;

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
