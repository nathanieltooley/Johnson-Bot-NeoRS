use rspotify::{ ClientCredsSpotify, Credentials };
use crate::custom_types::command::Error as CmdError;
use url::Url;
use std::error::Error;

#[derive(Debug)]
pub enum SpotifyParseError {
    InvalidDomain
}

impl Error for SpotifyParseError {}
impl std::fmt::Display for SpotifyParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpotifyParseError::InvalidDomain => {
                write!(f, "The provided URL has an invalid domain type")
            }
        }
    }
}

pub async fn spotify_init() -> Result<ClientCredsSpotify, CmdError>  {
    let creds = Credentials::from_env().expect("spotify creds envs should exist");
    let spotify = ClientCredsSpotify::new(creds);
    spotify.request_token().await?; 

    Ok(spotify)
}

pub fn url_to_uri(url: &Url) -> Result<String, SpotifyParseError> {
    let mut uri = String::from("spotify:");
    if let Some(domain) = url.domain() {
        if domain.to_string().as_str() != "open.spotify.com" {
            return Err(SpotifyParseError::InvalidDomain);
        }
    }

    let splits: Vec<&str> = url.path().split('/').collect();
    let spotify_type = splits[1];
    let id = splits[2];

    uri.push_str(&format!("{}:", spotify_type));
    uri.push_str(id);

    Ok(uri)
}
