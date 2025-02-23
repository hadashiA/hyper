#![feature(async_await)]
#![deny(warnings)]

use hyper::{Body, Chunk, Client, Method, Request, Response, Server, StatusCode, header};
use hyper::client::HttpConnector;
use hyper::service::{make_service_fn, service_fn};
use futures_util::{TryStreamExt};

type GenericError = Box<dyn std::error::Error + Send + Sync>;
type Result<T> = std::result::Result<T, GenericError>;

static INDEX: &[u8] = b"<a href=\"test.html\">test.html</a>";
static INTERNAL_SERVER_ERROR: &[u8] = b"Internal Server Error";
static NOTFOUND: &[u8] = b"Not Found";
static POST_DATA: &str = r#"{"original": "data"}"#;
static URL: &str = "http://127.0.0.1:1337/json_api";

async fn client_request_response(
    client: &Client<HttpConnector>
) -> Result<Response<Body>> {
    let req = Request::builder()
        .method(Method::POST)
        .uri(URL)
        .header(header::CONTENT_TYPE, "application/json")
        .body(POST_DATA.into())
        .unwrap();

    let web_res = client.request(req).await?;
    // Compare the JSON we sent (before) with what we received (after):
    let body = Body::wrap_stream(web_res.into_body().map_ok(|b| {
        Chunk::from(format!("<b>POST request body</b>: {}<br><b>Response</b>: {}",
                            POST_DATA,
                            std::str::from_utf8(&b).unwrap()))
    }));

    Ok(Response::new(body))
}

async fn api_post_response(req: Request<Body>) -> Result<Response<Body>> {
    // A web api to run against
    let entire_body = req.into_body().try_concat().await?;
    // TODO: Replace all unwraps with proper error handling
    let str = String::from_utf8(entire_body.to_vec())?;
    let mut data : serde_json::Value = serde_json::from_str(&str)?;
    data["test"] = serde_json::Value::from("test_value");
    let json = serde_json::to_string(&data)?;
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json))?;
    Ok(response)
}

async fn api_get_response() -> Result<Response<Body>> {
    let data = vec!["foo", "bar"];
    let res = match serde_json::to_string(&data) {
        Ok(json) => {
            Response::builder()
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json))
                .unwrap()
        }
        Err(_) => {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(INTERNAL_SERVER_ERROR.into())
                .unwrap()
        }
    };
    Ok(res)
}

async fn response_examples(
    req: Request<Body>,
    client: Client<HttpConnector>
) -> Result<Response<Body>> {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/") |
        (&Method::GET, "/index.html") => {
            Ok(Response::new(INDEX.into()))
        },
        (&Method::GET, "/test.html") => {
            client_request_response(&client).await
        },
        (&Method::POST, "/json_api") => {
            api_post_response(req).await
        },
        (&Method::GET, "/json_api") => {
            api_get_response().await
        }
        _ => {
            // Return 404 not found response.
            Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(NOTFOUND.into())
                .unwrap())
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();

    let addr = "127.0.0.1:1337".parse().unwrap();

    // Share a `Client` with all `Service`s
    let client = Client::new();

    let new_service = make_service_fn(move |_| {
        // Move a clone of `client` into the `service_fn`.
        let client = client.clone();
        async {
            Ok::<_, GenericError>(service_fn(move |req| {
                // Clone again to ensure that client outlives this closure.
                response_examples(req, client.to_owned())
            }))
        }
    });

    let server = Server::bind(&addr)
        .serve(new_service);

    println!("Listening on http://{}", addr);

    server.await?;

    Ok(())
}
