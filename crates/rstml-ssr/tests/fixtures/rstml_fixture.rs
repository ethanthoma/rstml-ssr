use super::{Assets, CAPTCHA_CONFIGURATION, PageData, mark_geometry};
use rstml_ssr::{Render, TrustedHtml, each, html};

pub fn page<'a>(
    assets: &'a Assets,
    meta: PageData<'a>,
    body: impl Render + 'a,
) -> impl Render + 'a {
    html! {
        <!DOCTYPE html>
        <html
            lang="en"
            class={if meta.url == "https://matterstateresearch.com/" {
                "bg-home-paper"
            } else {
                "bg-bg-1"
            }}
        >
            {head(assets, meta)}
            <body
                data-prefetch
                data-home={(matches!(
                    meta.url,
                    "https://matterstateresearch.com/" | "https://matterstateresearch.com/needle"
                ) || meta.url.starts_with("https://matterstateresearch.com/reports/"))
                    .then_some("")}
                class="flex flex-col antialiased min-h-dvh font-sans text-m bg-bg-1 text-text-2 wrap-anywhere"
            >
                <a
                    href="#main-content"
                    class="sr-only focus:absolute focus:z-50 focus:text-text-1 focus:no-underline focus:bg-bg-2 focus:border-2 focus:border-border-1 focus:not-sr-only focus:top-s focus:left-s focus:px-s focus:py-xs"
                >
                    "Skip to main content"
                </a>
                {header(
                    meta
                        .navigation_path
                        .unwrap_or_else(|| {
                            meta.url.strip_prefix("https://matterstateresearch.com").unwrap_or("/")
                        }),
                )}
                <aside
                    aria-label="Release announcement"
                    class="px-content py-xs bg-terminal-yellow text-terminal-black flex flex-wrap items-center justify-between gap-xs font-mono text-s leading-relaxed"
                >
                    <p class="flex-1 basis-[16ch] min-w-0">"Needle release and API coming soon."</p>
                    <a
                        href="/needle#updates"
                        class="inline-flex min-w-0 items-center gap-xs min-h-tap underline underline-offset-4 focus-visible:outline-terminal-black"
                    >
                        "Get updates"
                        {arrow_right()}
                    </a>
                </aside>
                {body}
                {footer()}
                <script type="module" src="/static/js/datastar-csp.js" />
                <script type="module" src="/static/js/datastar-scoped-signals.js" />
                <script type="module" src="/static/js/datastar-prefetch.js" />
                <script defer src={assets.url("js/sw-register.js")} />
                {if meta.drafts {
                    html! {
                        <script type="module" src={assets.url("js/form-drafts.js")} />
                        <script type="module" src={assets.url("js/form-errors.js")} />
                        <script type="module" src={assets.url("js/captcha-submit.js")} />
                    }
                } else {
                    ""
                }}
                {if assets.development {
                    html! { <script type="module" src={assets.url("js/dev-reload.js")} /> }
                } else {
                    ""
                }}
            </body>
        </html>
    }
}

fn head<'a>(assets: &'a Assets, meta: PageData<'a>) -> impl Render + 'a {
    let social_image = format!(
        "https://matterstateresearch.com{}",
        assets.url("imgs/og-card.png")
    );
    html! {
        <head>
            <meta charset="utf-8"/>
            <meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover, interactive-widget=resizes-content"/>
            {if meta.drafts { html! { <link rel="stylesheet" href="/captcha/assets/altcha.css"/> } } else { "" }}
            {if meta.url == "https://matterstateresearch.com/" {
                html! { <meta name="theme-color" content="#f9f8f2"/> }
            } else {
                html! {
                    <meta name="theme-color" content="#1b1d1c" media="(prefers-color-scheme: dark)"/>
                    <meta name="theme-color" content="#f6f5f0" media="(prefers-color-scheme: light)"/>
                }
            }}
            <title>{meta.title}</title>
            <meta name="description" content={meta.description}/>
            <meta property="og:title" content={meta.title}/>
            <meta property="og:description" content={meta.description}/>
            <meta property="og:type" content="website"/>
            <meta property="og:url" content={meta.url}/>
            <meta property="og:image" content={social_image}/>
            <meta property="og:image:alt" content={meta.title}/>
            <meta name="twitter:card" content="summary_large_image"/>
            <meta name="twitter:title" content={meta.title}/>
            <meta name="twitter:description" content={meta.description}/>
            <meta name="twitter:image" content={social_image}/>
            <meta name="twitter:image:alt" content={meta.title}/>
            <link rel="canonical" href={meta.url}/>
            <link rel="icon" href={assets.url("favicon.ico")} sizes="32x32"/>
            <link rel="icon" href={assets.url("favicon-192.png")} type="image/png" sizes="192x192"/>
            <link rel="icon" href={assets.url("imgs/matter-state.svg")} type="image/svg+xml" sizes="any"/>
            <link rel="apple-touch-icon" href={assets.url("imgs/apple-touch-icon.png")}/>
            <link rel="manifest" href={assets.url("manifest.json")}/>
            <meta name="mobile-web-app-capable" content="yes"/>
            <meta name="apple-mobile-web-app-capable" content="yes"/>
            <meta name="apple-mobile-web-app-status-bar-style" content="black-translucent"/>
            <meta name="apple-mobile-web-app-title" content="Matter State Research"/>
            <link rel="preload" href={assets.url("css/output.css")} as="style" fetchpriority="high"/>
            <link rel="stylesheet" href={assets.url("css/output.css")}/>
            <script src={assets.url("js/navigation-motion.js")}/>
            {each(meta.styles, |style| html! { <link rel="stylesheet" href={assets.url(style)}/> })}
            {match meta.schema { Some(schema) => TrustedHtml(schema), None => "" }}
        </head>
    }
}

const NAVIGATION_LINKS: [(&str, &str); 3] = [
    ("Work", "/#work"),
    ("About", "/#about"),
    ("Contact", "/contact"),
];

fn header(path: &str) -> impl Render + '_ {
    html! {
        <header
            data-site-header
            class="mx-content flex flex-wrap items-center justify-between gap-2xs-xs pt-[max(var(--spacing-s),env(safe-area-inset-top))] pb-xs"
        >
            <a
                href="/"
                class="inline-flex min-w-0 items-center min-h-[2.75rem] gap-4xs-2xs w-fit text-text-1"
            >
                {institution_mark()}
                <span class="block min-w-0 w-fit font-serif text-masthead font-normal leading-tight tracking-tighter">
                    "Matter State "<br />"Research"
                </span>
            </a>
            <nav aria-label="Main" class="site-navigation ml-auto min-w-0 max-w-full w-fit">
                <ul class="flex flex-wrap justify-end font-mono text-navigation font-medium leading-snug">
                    {each(
                        &NAVIGATION_LINKS,
                        |(label, href)| {
                            html! {
                                <li>
                                    <a
                                        href={href}
                                        class="inline-flex items-center justify-center min-h-[3rem] min-w-[3rem] px-3xs-xs py-2xs"
                                        aria-current={navigation_current(href, path)}
                                    >
                                        {label}
                                    </a>
                                </li>
                            }
                        },
                    )}
                </ul>
            </nav>
        </header>
    }
}

fn institution_mark() -> impl Render {
    html! {
        <svg
            viewBox="-6 -6 112 112"
            width="32"
            height="32"
            aria-hidden="true"
            focusable="false"
            fill="currentColor"
            class="size-institution-mark shrink-0"
        >
            {TrustedHtml(mark_geometry())}
        </svg>
    }
}

fn footer() -> impl Render {
    html! {
        <footer
            data-site-footer
            class="mx-content pb-safe-m flex flex-wrap items-baseline justify-between gap-s border-t border-border-2 pt-s text-xs leading-relaxed text-text-3"
        >
            <p>"Matter State Research"</p>
            <div class="flex flex-wrap items-center gap-x-m gap-y-xs">
                <p>"Research, software, and engineering."</p>
                <nav
                    aria-label="Footer"
                    class="site-footer-navigation flex flex-wrap items-center gap-x-m gap-y-xs"
                >
                    <a href="/contact" class="inline-flex items-center min-h-tap">
                        "Contact"
                    </a>
                    <a
                        href="https://github.com/matterstate"
                        class="inline-flex items-center min-h-tap"
                    >
                        "GitHub ↗"
                    </a>
                    <a
                        href="https://huggingface.co/matterstate"
                        class="inline-flex items-center min-h-tap"
                    >
                        "Hugging Face ↗"
                    </a>
                </nav>
            </div>
        </footer>
    }
}

fn arrow_right() -> impl Render {
    html! {
        <svg
            aria-hidden="true"
            class="w-[1em] h-[1em]"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
        >
            <path d="M5 12h14M12 5l7 7-7 7" />
        </svg>
    }
}

fn navigation_current(href: &str, path: &str) -> Option<&'static str> {
    match href {
        "/#work"
            if path == "/needle" || path.starts_with("/reports") || path.starts_with("/notes") =>
        {
            Some("location")
        }
        "/#about" if path == "/about" => Some("location"),
        "/contact" if path == "/contact" => Some("page"),
        _ => None,
    }
}

pub fn contact_form() -> impl Render {
    html! {
        <form
            method="post"
            action="/contact"
            data-draft="contact"
            data-draft-fields="name email message"
            class="flex flex-col w-full gap-s"
            data-indicator:fetching
            data-attr:aria-busy="$fetching"
            data-on:submit="@post('/contact', {contentType: 'form'})"
        >
            <p data-form-error role="alert" aria-atomic="true" class="form-notice w-full sr-only" />
            {field("name", "Name", required_input("name", "text", "name"))}
            {field("email", "Email", required_input("email", "email", "email"))}
            {field(
                "message",
                "Message",
                html! {
                    <textarea
                        id="message"
                        name="message"
                        required
                        rows="6"
                        class="form-control resize-y text-m leading-relaxed px-xs py-2xs"
                    />
                },
            )}
            {captcha_submit()}
        </form>
    }
}

fn field<'a>(id: &'a str, label: &'a str, input: impl Render + 'a) -> impl Render + 'a {
    html! {
        <div class="flex flex-col gap-3xs">
            <label for={id} class="font-mono text-xs text-text-3">
                {label}
            </label>
            {input}
        </div>
    }
}

fn required_input<'a>(id: &'a str, kind: &'a str, autocomplete: &'a str) -> impl Render + 'a {
    html! {
        <input
            id={id}
            type={kind}
            name={id}
            required
            autocomplete={autocomplete}
            class="form-control min-h-tap text-m leading-snug px-xs py-2xs"
        />
    }
}

fn captcha_submit() -> impl Render {
    html! {
        <div
            data-captcha-submit
            data-captcha-state="verifying"
            class="grid grid-cols-1 grid-rows-1 items-start w-full min-h-tap"
        >
            <altcha-widget
                id={None::<&str>}
                data-captcha-widget
                data-ignore-morph
                challenge="/captcha/challenge?action=contact"
                configuration={CAPTCHA_CONFIGURATION}
                class="col-start-1 row-start-1 w-full max-w-full font-mono text-s font-semibold [--altcha-checkbox-size:calc(var(--spacing-tap)/2)] [--altcha-padding:calc((var(--spacing-tap)_-_var(--altcha-checkbox-size)_-_2px)/2)_var(--spacing-s)] [--altcha-border-radius:var(--radius-lg)] [--altcha-color-base:var(--color-bg-1)] [--altcha-color-base-content:var(--color-text-1)] [--altcha-color-primary:var(--color-text-1)] [--altcha-color-primary-content:var(--color-bg-1)] [--altcha-border-color:var(--color-border-2)] [--altcha-max-width:100%]"
            />
            <div
                data-captcha-button
                inert
                class="col-start-1 row-start-1 self-center min-w-0 w-full invisible"
            >
                <button
                    type="submit"
                    disabled
                    data-variant="standard"
                    class="form-submit inline-flex min-h-tap w-full min-w-0 items-center justify-center gap-2xs text-s font-semibold px-s py-2xs"
                >
                    "Send message"
                    {arrow_right()}
                </button>
            </div>
            <p
                data-captcha-status
                role="status"
                aria-atomic="true"
                class="col-start-1 row-start-2 text-s text-text-1 min-h-[2lh] mt-2xs"
            >
                "Verification is loading."
            </p>
        </div>
        <noscript>
            <p class="w-full text-s text-text-2">
                "JavaScript is required to verify and submit this form."
            </p>
        </noscript>
    }
}
