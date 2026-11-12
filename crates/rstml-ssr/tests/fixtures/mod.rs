pub mod maud_fixture;
pub mod rstml_fixture;

#[derive(Clone, Copy)]
pub struct PageData<'a> {
    pub title: &'a str,
    pub description: &'a str,
    pub drafts: bool,
    pub url: &'a str,
    pub schema: Option<&'a str>,
    pub navigation_path: Option<&'a str>,
    pub styles: &'a [&'a str],
}

pub struct Assets {
    pub development: bool,
}

impl Assets {
    pub fn url(&self, name: &str) -> String {
        format!("/static/{name}?v=fixture")
    }
}

pub const CAPTCHA_CONFIGURATION: &str = r#"{"auto":"off","disableAutoFocus":true,"display":"standard","hideFooter":true,"hideLogo":true,"humanInteractionSignature":false,"minDuration":0,"serverVerificationFields":false,"serverVerificationTimeZone":false,"timeout":10000,"type":"native","workers":1}"#;

pub fn mark_geometry() -> &'static str {
    include_str!("matter-state.svg")
        .split_once("</style>")
        .unwrap()
        .1
        .trim_end()
        .strip_suffix("</svg>")
        .unwrap()
}
