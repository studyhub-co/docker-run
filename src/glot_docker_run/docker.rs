use http;
use crate::glot_docker_run::http_extra;
use serde::{Serialize, Deserialize};
use serde_json;
use std::io::{Read, Write};
use std::io;
use std::convert::TryInto;

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ContainerConfig {
    hostname: String,
    //domain_name: String,
    user: String,
    attach_stdin: bool,
    attach_stdout: bool,
    attach_stderr: bool,
    tty: bool,
    open_stdin: bool,
    stdin_once: bool,
    //cmd: Vec<String>,
    //entrypoint: Vec<String>,
    image: String,
    network_disabled: bool,
    host_config: HostConfig,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct HostConfig {
    //cpu_shared: i32,
    memory: i64,
    privileged: bool,
    cap_add: Vec<String>,
    cap_drop: Vec<String>,
    ulimits: Vec<Ulimit>,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Ulimit {
    name: String,
    soft: i64,
    hard: i64,
}

pub fn default_container_config(image_name: String) -> ContainerConfig {
    ContainerConfig{
        hostname: "glot-runner".to_string(),
        user: "glot".to_string(),
        attach_stdin: true,
        attach_stdout: true,
        attach_stderr: true,
        tty: false,
        open_stdin: true,
        stdin_once: true,
        //cmd: Vec<String>,
        //entrypoint: Vec<String>,
        image: image_name,
        network_disabled: true,
        host_config: HostConfig{
            memory: 500000000,
            privileged: false,
            cap_add: vec![],
            cap_drop: vec!["MKNOD".to_string()],
            ulimits: vec![
                Ulimit{
                    name: "nofile".to_string(),
                    soft: 90,
                    hard: 100,
                },
                Ulimit{
                    name: "nproc".to_string(),
                    soft: 90,
                    hard: 100,
                },
            ],
        },
    }
}


#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct VersionResponse {
    version: String,
    api_version: String,
    kernel_version: String,
}

pub fn version_request() -> http::Request<http_extra::Body> {
    http::Request::get("/version")
        .header("Accept", "application/json")
        .header("Host", "127.0.0.1")
        .header("Connection", "close")
        .body(http_extra::Body::Empty())
        .unwrap()
}

pub fn version<Stream: Read + Write>(mut stream: Stream) -> Result<http::Response<VersionResponse>, io::Error> {
    let req = version_request();
    http_extra::send_request(stream, req)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ContainerCreatedResponse {
    id: String,
    warnings: Vec<String>,
}


pub fn create_container(config: &ContainerConfig) -> http::Request<http_extra::Body> {
    let body = serde_json::to_vec(config).unwrap();

    http::Request::post("/containers/create")
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .header("Host", "127.0.0.1")
        .header("Content-Length", body.len())
        .header("Connection", "close")
        .body(http_extra::Body::Bytes(body))
        .unwrap()
}


pub fn start_container(containerId: &str) -> http::Request<http_extra::Body> {
    let url = format!("/containers/{}/start", containerId);

    http::Request::post(url)
        .header("Accept", "application/json")
        .header("Host", "127.0.0.1")
        .header("Connection", "close")
        .body(http_extra::Body::Empty())
        .unwrap()
}


pub fn attach_container_request(containerId: &str) -> http::Request<http_extra::Body> {
    let url = format!("/containers/{}/attach?stream=1&stdout=1&stdin=1&stderr=1", containerId);

    http::Request::post(url)
        .header("Host", "127.0.0.1")
        .body(http_extra::Body::Empty())
        .unwrap()
}

pub fn attach_container<Stream: Read + Write>(stream: Stream, containerId: &str) -> Result<http::Response<http_extra::EmptyResponse>, io::Error> {
    let req = attach_container_request(containerId);
    http_extra::send_request(stream, req)
}

// TODO: this is a glot specific function
pub fn attach_and_send_payload<Stream, Payload>(mut stream: Stream, containerId: &str, payload: Payload) -> Result<StreamResult, StreamError>
    where
        Stream: Read + Write,
        Payload: Serialize,
    {

    attach_container(&mut stream, containerId);

    // Send payload
    serde_json::to_writer(&mut stream, &payload);

    // Read response
    read_stream(stream)
}



#[derive(Debug)]
pub enum StreamError {
    Read(io::Error),
}


type StreamResult = Result<Vec<u8>, Vec<u8>>;


pub fn read_stream<R: Read>(mut r: R) -> Result<StreamResult, StreamError> {
    let mut reader = iowrap::Eof::new(r);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    while !reader.eof().map_err(StreamError::Read)? {
        let stream_type = read_stream_type(&mut reader);
        let stream_length = read_stream_length(&mut reader);

        let mut buffer = vec![0u8; stream_length];
        reader.read_exact(&mut buffer);

        match stream_type {
            StreamType::Stdin() => {

            }

            StreamType::Stdout() => {
                stdout.append(&mut buffer);
            }

            StreamType::Stderr() => {
                stderr.append(&mut buffer);
            }
        }
    }

    if stderr.len() > 0 {
        Ok(Err(stderr))
    } else {
        Ok(Ok(stdout))
    }
}


#[derive(Debug)]
enum StreamType {
    Stdin(),
    Stdout(),
    Stderr(),
}

impl StreamType {
    fn from_byte(n: u8) -> Option<StreamType> {
        match n {
            0 => Some(StreamType::Stdin()),
            1 => Some(StreamType::Stdout()),
            2 => Some(StreamType::Stderr()),
            _ => None,
        }
    }
}

fn read_stream_type<R: Read>(mut reader: R) -> StreamType {
    let mut buffer = [0; 4];
    reader.read_exact(&mut buffer);

    StreamType::from_byte(buffer[0]).unwrap()
}

fn read_stream_length<R: Read>(mut reader: R) -> usize {
    let mut buffer = [0; 4];
    reader.read_exact(&mut buffer);

    u32::from_be_bytes(buffer).try_into().unwrap()
}


