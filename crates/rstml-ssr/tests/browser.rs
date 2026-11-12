use std::{
    env, fs,
    io::{self, Read},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

use rstml_ssr::{html, render};
use serde_json::{Value, json};
use tiny_http::{Header, Method, Request, Response, Server};

const NOTE: &str = "\"'&<> é🦀";
const PATCH_TEXT: &str =
    "first line\n\n\"'&<img src=x onerror=\"globalThis.test_xss_fired=true\"> é🦀\r\nlast\r";
const SUBMIT_EXPRESSION: &str = "@post('/patch', {contentType: 'form'})";

#[test]
#[ignore = "requires Chromium and RSTML_SSR_DATASTAR_JS pointing to a Datastar module"]
fn browser_events_requests_and_sse_patches() {
    let datastar_path = env::var_os("RSTML_SSR_DATASTAR_JS").expect("set RSTML_SSR_DATASTAR_JS");
    let mut datastar = Vec::new();
    fs::File::open(datastar_path)
        .unwrap()
        .take(1_048_577)
        .read_to_end(&mut datastar)
        .unwrap();
    assert!(datastar.len() <= 1_048_576);
    let fixture = Fixture::new(datastar);
    let server = Server::http("127.0.0.1:0").unwrap();
    let url = format!("http://{}", server.server_addr());
    let stopped = Arc::new(AtomicBool::new(false));
    let server_stopped = stopped.clone();
    let (sender, receiver) = mpsc::sync_channel(1);
    let directory = env::temp_dir().join(format!("rstml-ssr-browser-{}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    let log = fs::File::create(directory.join("chromium.log")).unwrap();
    let worker = thread::spawn(move || serve(server, fixture, server_stopped, sender));
    let chromium = env::var_os("RSTML_SSR_CHROMIUM").unwrap_or_else(|| "chromium".into());
    let outcome = Command::new(chromium)
        .args([
            "--headless",
            "--disable-gpu",
            "--disable-dev-shm-usage",
            "--disable-background-networking",
            "--no-first-run",
            "--no-default-browser-check",
            "--enable-logging=stderr",
        ])
        .arg(format!(
            "--user-data-dir={}",
            directory.join("profile").display()
        ))
        .arg(url)
        .stdout(Stdio::null())
        .stderr(Stdio::from(log))
        .spawn()
        .and_then(|mut browser| {
            let result = receiver.recv_timeout(Duration::from_secs(20));
            if browser.try_wait()?.is_none() {
                browser.kill()?;
            }
            browser.wait()?;
            result.map_err(io::Error::other)
        });
    stopped.store(true, Ordering::Relaxed);
    let requests = worker.join();
    let mut diagnostics = String::new();
    fs::File::open(directory.join("chromium.log"))
        .unwrap()
        .take(65_536)
        .read_to_string(&mut diagnostics)
        .unwrap();
    fs::remove_dir_all(&directory).unwrap();
    let requests = requests.unwrap().unwrap();
    let report = outcome.unwrap_or_else(|error| panic!("{error}\n{diagnostics}"));
    assert_eq!(report["passed"], true, "{report:#}\n{diagnostics}");
    assert_eq!(requests.patch_count, 1);
    assert_eq!(requests.followup_count, 1);
    println!(
        "{} browser checks passed: {}",
        report["checks"].as_array().unwrap().len(),
        report["browser"]
    );
}

struct Fixture {
    datastar: Vec<u8>,
    page: String,
    expected: String,
    patch: String,
    followup: String,
}

impl Fixture {
    fn new(datastar: Vec<u8>) -> Self {
        let page = render(&html! {
            <!DOCTYPE html>
            <html lang="en">
                <head>
                    <meta charset="utf-8" />
                    <title>"Datastar browser fixture"</title>
                </head>
                <body data-signals="{\"fetching\":false}">
                    <svg viewBox="0 0 10 10">
                        <path id="inline-path" d="M0 0L10 10" />
                        <script>"globalThis.foreign_script_checked = 1 < 2 && 'SVG&A';"</script>
                        <style>".foreign-check > span { color: rgb(12, 34, 56); }"</style>
                    </svg>
                    <math>
                        <mtext>
                            <mglyph id="math-glyph" />
                            <span id="math-text">"Text"</span>
                        </mtext>
                        <annotation-xml encoding="text/html">
                            <p id="math-html">"HTML"<br /></p>
                        </annotation-xml>
                        <annotation-xml>
                            <svg id="math-svg">
                                <foreignObject>
                                    <p id="math-svg-html">"Embedded"</p>
                                </foreignObject>
                            </svg>
                        </annotation-xml>
                    </math>
                    <style>".raw-check > span { color: rgb(12, 34, 56); }"</style>
                    <p class="raw-check">
                        <span id="inline-style">"Styled"</span>
                    </p>
                    <p class="foreign-check">
                        <span id="foreign-style">"Styled"</span>
                    </p>
                    <script>"globalThis.inline_script_checked = 1 < 2 && 'A&B';"</script>
                    <form
                        id="patch-form"
                        data-on:submit={SUBMIT_EXPRESSION}
                        data-indicator:fetching
                        data-attr:aria-busy="$fetching"
                    >
                        <input id="note" name="note" value={NOTE} />
                        <button id="send" type="submit" data-attr:disabled="$fetching">
                            "Send patch"
                        </button>
                    </form>
                    <p id="aria-status" data-attr:aria-busy="$fetching ? 'true' : 'false'">
                        "Status"
                    </p>
                    <section id="result" hidden data-empty="old" data-optional="old">
                        "Before"
                    </section>
                    <p id="confirmation" data-round=0_u32>
                        "Waiting"
                    </p>
                    <script type="module" src="/checks.js" />
                </body>
            </html>
        })
        .unwrap()
        .into_string();
        let patch = render(&html! {
            <section
                id="result"
                data-round=1_u32
                title={NOTE}
                data-empty={Some("")}
                data-optional={None::<&str>}
                hidden={false}
            >
                <h2>"Patched"</h2>
                <pre id="payload">{PATCH_TEXT}</pre>
                <button id="followup" data-on:click="@get('/followup')" disabled={false}>
                    "Follow up"
                </button>
            </section>
        })
        .unwrap()
        .into_string();
        let followup = render(&html! {
            <p id="confirmation" data-round=2_u32>
                "Second event worked"
            </p>
        })
        .unwrap()
        .into_string();
        Self {
            datastar,
            page,
            expected:
                json!({ "note": NOTE, "text": PATCH_TEXT, "submit_expression": SUBMIT_EXPRESSION })
                    .to_string(),
            patch: frame(&patch),
            followup: frame(&followup),
        }
    }
}

#[derive(Default)]
struct RequestCounts {
    patch_count: u32,
    followup_count: u32,
}

fn serve(
    server: Server,
    fixture: Fixture,
    stopped: Arc<AtomicBool>,
    sender: mpsc::SyncSender<Value>,
) -> io::Result<RequestCounts> {
    let mut request_count = 0_u32;
    let mut requests = RequestCounts::default();
    for _ in 0..300_u32 {
        if stopped.load(Ordering::Relaxed) {
            break;
        }
        let Some(mut request) = server.recv_timeout(Duration::from_millis(100))? else {
            continue;
        };
        request_count += 1;
        assert!(request_count <= 32);
        let route = request.url().split('?').next().unwrap().to_owned();
        match route.as_str() {
            "/" => respond(request, "text/html; charset=utf-8", fixture.page.as_bytes())?,
            "/datastar.js" => {
                respond(request, "text/javascript; charset=utf-8", &fixture.datastar)?
            }
            "/checks.js" => respond(
                request,
                "text/javascript; charset=utf-8",
                include_bytes!("fixtures/browser-checks.js"),
            )?,
            "/expected" => respond(request, "application/json", fixture.expected.as_bytes())?,
            "/patch" => {
                assert_eq!(request.method(), &Method::Post);
                assert!(
                    request
                        .headers()
                        .iter()
                        .any(|header| header.field.equiv("Datastar-Request")
                            && header.value.as_str() == "true")
                );
                let mut body = String::new();
                request.as_reader().take(8193).read_to_string(&mut body)?;
                assert!(body.len() <= 8192);
                assert_eq!(body, expected_form_body());
                requests.patch_count += 1;
                thread::sleep(Duration::from_millis(150));
                respond(request, "text/event-stream", fixture.patch.as_bytes())?;
            }
            "/followup" => {
                assert_eq!(request.method(), &Method::Get);
                requests.followup_count += 1;
                respond(request, "text/event-stream", fixture.followup.as_bytes())?;
            }
            "/report" => {
                let mut body = String::new();
                request.as_reader().take(8193).read_to_string(&mut body)?;
                assert!(body.len() <= 8192);
                let report = serde_json::from_str(&body)?;
                respond(request, "text/plain", b"recorded")?;
                sender.send(report).map_err(io::Error::other)?;
            }
            _ => request.respond(Response::empty(404))?,
        }
    }
    Ok(requests)
}

fn respond(request: Request, content_type: &str, body: &[u8]) -> io::Result<()> {
    let header = Header::from_bytes("Content-Type", content_type).unwrap();
    request.respond(Response::from_data(body).with_header(header))
}

fn frame(markup: &str) -> String {
    use std::fmt::Write;
    let mut frame = String::from("event: datastar-patch-elements\ndata: mode replace\n");
    for line in markup.split('\n') {
        writeln!(frame, "data: elements {line}").unwrap();
    }
    frame.push('\n');
    frame
}

fn expected_form_body() -> String {
    use std::fmt::Write;
    let mut body = String::from("note=");
    for byte in NOTE.bytes() {
        match byte {
            b' ' => body.push('+'),
            b'*' | b'-' | b'.' | b'_' => body.push(char::from(byte)),
            byte if byte.is_ascii_alphanumeric() => body.push(char::from(byte)),
            _ => write!(body, "%{byte:02X}").unwrap(),
        }
    }
    body
}
