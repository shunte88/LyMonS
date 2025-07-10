use serde::{Serialize, Deserialize};
use serde_json::{json, Value, Error as SerdeJsonError};
use reqwest::{Client, header, Error as ReqwestError};
use std::fmt::{self, Display, Formatter};
use std::time::Duration;

/// Custom error type for SlimInfoClient operations.
#[derive(Debug)]
pub enum SlimInfoClientError {
    /// Error during HTTP request (e.g., network issues, invalid URL).
    HttpRequestError(ReqwestError),
    /// Error serializing the request payload to JSON.
    SerializationError(SerdeJsonError),
    /// Error deserializing the response payload from JSON.
    DeserializationError(SerdeJsonError),
    /// The LMS Server response contained an error object.
    RpcError(RpcError),
    /// The LMS Server response was missing the 'result' field when expected.
    MissingResult,
    /// The LMS Server response was missing the 'id' field when expected.
    MissingId,
    /// Mismatched ID between request and response.
    IdMismatch { expected: u32, received: Option<u32> },
}

impl Display for SlimInfoClientError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            SlimInfoClientError::HttpRequestError(e) => write!(f, "HTTP request error: {}", e),
            SlimInfoClientError::SerializationError(e) => write!(f, "JSON serialization error: {}", e),
            SlimInfoClientError::DeserializationError(e) => write!(f, "JSON deserialization error: {}", e),
            SlimInfoClientError::RpcError(e) => write!(f, "LMS Server error: {:?}", e),
            SlimInfoClientError::MissingResult => write!(f, "LMS Server response missing 'result' field"),
            SlimInfoClientError::MissingId => write!(f, "LMS Server response missing 'id' field"),
            SlimInfoClientError::IdMismatch { expected, received } => {
                write!(f, "LMS Server ID mismatch: expected {}, received {:?}", expected, received)
            }
        }
    }
}

impl std::error::Error for SlimInfoClientError {}

impl From<ReqwestError> for SlimInfoClientError {
    fn from(err: ReqwestError) -> Self {
        SlimInfoClientError::HttpRequestError(err)
    }
}

impl From<SerdeJsonError> for SlimInfoClientError {
    fn from(err: SerdeJsonError) -> Self {
        SlimInfoClientError::SerializationError(err) // Default to serialization, can be refined later
    }
}

/// Represents the custom SlimInfo request payload.
#[derive(Debug, Serialize)]
pub struct SlimRequest {
    pub id: u32, // call id
    pub method: String, // Fixed "slim.request"
    #[serde(rename = "params")]
    pub params: Vec<Value>, // (player MAC, [param,...])
}

/// Represents a standard LMS Server error object.
#[derive(Debug, Deserialize)]
pub struct RpcError {
    #[allow(dead_code)]
    pub code: i32,
    #[allow(dead_code)]
    pub message: String,
    #[allow(dead_code)]
    pub data: Option<Value>,
}

/// Represents a standard LMS Server response structure.
#[derive(Debug, Deserialize)]
pub struct JsonRpcResponse { // This struct name remains as it represents the *format* of the response
    pub id: Option<u32>,
    pub result: Option<Value>,
    pub error: Option<RpcError>,
}

/// A client for interacting with a modified LMS Server service.
#[derive(Debug)]
pub struct SlimInfoClient {
    id: u32,
    method: String,
    client: Client,
}

impl SlimInfoClient {
    /// Creates a new `SlimInfoClient` instance inclusive populated headers and timeout.
    pub fn new() -> Self {
        // Define a constant for the User-Agent version
        const VERSION: &'static str = concat!("LyMonS ",env!("CARGO_PKG_NAME"), " v", env!("CARGO_PKG_VERSION"));
        const SLIM_REQUEST: &'static str = "slim.request";
        
        let mut headers = header::HeaderMap::new();
        headers.insert("User-Agent", header::HeaderValue::from_static(VERSION));
        headers.insert("Content-Type", header::HeaderValue::from_static("application/json"));
        headers.insert("Accept", header::HeaderValue::from_static("application/json"));
        headers.insert("Connection", header::HeaderValue::from_static("close"));
    
        let client = Client::builder()
            .http1_only()
            .connect_timeout(Duration::from_millis(500))
            .default_headers(headers)
            .timeout(Duration::from_millis(800))
            .build()
            .unwrap(); // Panics if client cannot be built, which is acceptable for client initialization

        SlimInfoClient {
            id: 1, // Start with ID 1, will increment for each request
            method: SLIM_REQUEST.to_string(),
            client, // Use the built client
        }
    }

    /// Sends a POST request to a LMS Server service with a SlimRequest payload.
    ///
    /// # Arguments
    /// * `host` - The hostname or IP address of the LMS Server service.
    /// * `port` - The port number of the LMS Server service.
    /// * `player_mac` - The player MAC address (or "-" if not player-specific).
    /// * `command` - The specific command string for the service.
    /// * `inner_params` - A vector of `serde_json::Value` representing the command-specific parameters.
    ///
    /// # Returns
    /// A `Result` which is `Ok(serde_json::Value)` containing the JSON result from the LMS Server
    /// call on success, or `Err(SlimInfoClientError)` on failure.
    pub async fn send_slim_request(
        &mut self,
        host: &str,
        port: u16,
        player_mac: &str,
        command: &str,
        inner_params: Vec<Value>,
    ) -> Result<Value, SlimInfoClientError> {

        let current_request_id = self.id; // Get the current ID for this request
        self.id += 1; // Increment ID for the next request

        let url = format!("http://{}:{}/jsonrpc.js", host, port);

        // Construct the inner params array: [command, value, value...]
        let mut command_and_params = vec![json!(command)]; //.to_string())];
        command_and_params.extend(inner_params);

        // Construct the main params array: [player_mac, [command, value, value...]]
        let request_params = vec![
            Value::String(player_mac.to_string()),
            Value::Array(command_and_params),
        ];

        // Create the SlimRequest payload
        let slim_request = SlimRequest {
            id: current_request_id, // Use the current_request_id
            method: self.method.clone(),
            params: request_params,
        };

        // Serialize the request payload to JSON
        let request_body = serde_json::to_string(&slim_request)
            .map_err(SlimInfoClientError::SerializationError)?;

        // Send the POST request using the client from the struct
        let response = self.client // Use self.client
            .post(&url)
            .body(request_body)
            .send()
            .await?; // Converts ReqwestError to SlimInfoClientError

        // Check for HTTP status code
        response.error_for_status_ref()?;

        // Deserialize the response body
        let response_text = response.text().await?;

        let rpc_response: JsonRpcResponse = serde_json::from_str(&response_text)
            .map_err(SlimInfoClientError::DeserializationError)?;

        // Validate the response ID
        if rpc_response.id.is_none() {
            return Err(SlimInfoClientError::MissingId);
        }
        if rpc_response.id != Some(current_request_id) { // Compare with current_request_id
            return Err(SlimInfoClientError::IdMismatch {
                expected: current_request_id,
                received: rpc_response.id,
            });
        }

        // Check for RPC errors in the response
        if let Some(error) = rpc_response.error {
            return Err(SlimInfoClientError::RpcError(error));
        }

        // Return the result field
        rpc_response.result.ok_or(SlimInfoClientError::MissingResult)
    }
}

