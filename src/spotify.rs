use futures::{StreamExt, TryStreamExt};
use rspotify::{ clients::BaseClient, model::{FullTrack, IdError, PlaylistId, TrackId, Type, PlaylistItem}, ClientCredsSpotify, ClientError, Credentials };
use crate::custom_types::command::Error as CmdError;
use url::Url;
use std::error::Error;
use futures_util::pin_mut;

#[derive(Debug)]
pub enum JohnsonSpotifyError {
    InvalidDomain,
    IdError(IdError),
    UnsupportedIdType(Type),
    ClientError(ClientError)
}

impl Error for JohnsonSpotifyError {}
impl std::fmt::Display for JohnsonSpotifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JohnsonSpotifyError::InvalidDomain => {
                write!(f, "The provided URL has an invalid domain type")
            }
            JohnsonSpotifyError::IdError(e) => {
                write!(f, "A problem occured when trying to parse spotify ID: {}", e)
            }
            JohnsonSpotifyError::UnsupportedIdType(t) => {
                write!(f, "This type of Spotify ID is not supported for playback: {}", t)
            }
            JohnsonSpotifyError::ClientError(c) => {
                write!(f, "An error occured while trying to use the client: {}", c)
            }
        }
    }
}

impl From<IdError> for JohnsonSpotifyError {
    fn from(value: IdError) -> Self {
        JohnsonSpotifyError::IdError(value)
    }
}

impl From<ClientError> for JohnsonSpotifyError {
    fn from(value: ClientError) -> Self {
        JohnsonSpotifyError::ClientError(value)
    }
}

struct SpotifyURI {
    spotify_type: Type,
    id: String
}

impl SpotifyURI {
    pub fn from_uri_str(uri: &str) -> Result<SpotifyURI, JohnsonSpotifyError> {
        let (spotify_type, id) = rspotify::model::parse_uri(uri)?;
        Ok(SpotifyURI {spotify_type, id: id.to_string()})
    }

    pub fn from_url(url: &Url) -> Result<SpotifyURI, JohnsonSpotifyError> {
        let uri = url_to_uri_str(url)?;
        let (spotify_type, id) = rspotify::model::parse_uri(&uri)?;

        Ok(SpotifyURI {spotify_type, id: id.to_string()})
    }
}

pub async fn spotify_init() -> Result<ClientCredsSpotify, CmdError>  {
    let creds = Credentials::from_env().expect("spotify creds envs should exist");
    let spotify = ClientCredsSpotify::new(creds);
    spotify.request_token().await?; 

    Ok(spotify)
}

pub fn url_to_uri_str(url: &Url) -> Result<String, JohnsonSpotifyError> {
    let mut uri = String::from("spotify:");
    if let Some(domain) = url.domain() {
        if domain.to_string().as_str() != "open.spotify.com" {
            return Err(JohnsonSpotifyError::InvalidDomain);
        }
    }

    let splits: Vec<&str> = url.path().split('/').collect();
    let spotify_type = splits[1];
    let id = splits[2];

    uri.push_str(&format!("{}:", spotify_type));
    uri.push_str(id);

    Ok(uri)
}

pub async fn get_tracks_from_url(spotify_client: &ClientCredsSpotify, url: &Url) -> Result<Vec<FullTrack>, JohnsonSpotifyError> {
    let uri = SpotifyURI::from_url(url)?;

    match uri.spotify_type {
        Type::Track => {
            let track = spotify_client.track(TrackId::from_id(uri.id)?, None).await?; 
            Ok(vec![track])
        }
        Type::Playlist => {
            let playlist_stream = spotify_client.playlist_items(PlaylistId::from_id(uri.id)?, None, None);
            pin_mut!(playlist_stream);

            let items = playlist_stream.try_collect::<Vec<PlaylistItem>>().await?;

            let tracks: Vec<_> = items.into_iter().filter_map(|item| {
                // Will try and get the track value or return None
                // If there is a track, it will then return Some if the track is
                // an actual song and not a podcast episode
                item.track.and_then(|playable_item| match playable_item {
                    rspotify::model::PlayableItem::Track(full_track) => {
                        Some(full_track)
                    }
                    rspotify::model::PlayableItem::Episode(_) => None
                })
            }).collect();

            Ok(tracks)
        }
        _ => {
            Err(JohnsonSpotifyError::UnsupportedIdType(uri.spotify_type))
        }
    }
}
