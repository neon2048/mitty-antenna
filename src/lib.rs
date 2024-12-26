use scraper::ElementRef;
use serde::Deserialize;
use std::{collections::HashMap, fmt::Display, result::Result};
use thiserror::Error;
use worker::*;

#[derive(Error, Debug)]
pub enum MittyAntennaError {
    #[error("update board parsing error: {0}")]
    UpdateBoard(&'static str),

    #[error("worker error {0}")]
    Worker(#[from] worker::Error),

    #[error("reqwest error {0}")]
    Reqwest(#[from] reqwest::Error),
}

const TERMINAL_URL: &str = "https://mitty-terminal.uwu.ai/";

#[event(fetch)]
pub async fn main(_req: Request, _env: Env, _ctx: worker::Context) -> worker::Result<Response> {
    return Response::error("Not found", 404);
}

#[event(scheduled)]
async fn scheduled(req: ScheduledEvent, env: Env, ctx: ScheduleContext) {
    match check_terminal_updates(req, env, ctx).await {
        Ok(_) => {}
        Err(e) => {
            console_error!("{}", e);
        }
    }
}

async fn check_terminal_updates(
    _req: ScheduledEvent,
    env: Env,
    _ctx: ScheduleContext,
) -> Result<(), MittyAntennaError> {
    console_error_panic_hook::set_once();

    let webhook = env.secret("DISCORD_WEBHOOK")?.to_string();

    let body = get_terminal_body(TERMINAL_URL).await?;

    let updates = find_updates(&body).await?;

    let db = env.d1("DB")?;

    for update in updates.into_iter().rev() {
        let msg = format!("{}", update.body);

        let statement = db.prepare("SELECT body, title FROM MittyUpdates WHERE body = ?1");
        let query = statement.bind(&[update.body.clone().into()])?;
        let result = query.first::<MittyUpdate>(None).await?;

        match result {
            Some(_) => return Ok(()),
            None => {
                let _res = send_message(&webhook, &msg).await?;
                let statement =
                    db.prepare("INSERT INTO MittyUpdates (title, body) VALUES (?1, ?2)");
                let query = statement.bind(&[update.title.into(), update.body.into()])?;
                let _ = query.run().await?;
                console_log!("new message stored")
            }
        };
    }
    Ok(())
}

async fn send_message(webhook: &str, message: &str) -> Result<(), reqwest::Error> {
    let mut map = HashMap::new();
    map.insert("content", message);
    let client = reqwest::Client::new();

    let resp = client.post(webhook).json(&map).send().await?;

    match resp.error_for_status() {
        Ok(_) => return Ok(()),
        Err(e) => return Err(e),
    }
}

async fn get_terminal_body(url: &str) -> Result<String, reqwest::Error> {
    let client = reqwest::Client::new();
    let resp = client.get(url).send().await?;
    resp.text().await
}

#[derive(Deserialize)]
struct MittyUpdate {
    title: String,
    body: String,
}

impl Display for MittyUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.title, self.body)
    }
}

impl TryFrom<ElementRef<'_>> for MittyUpdate {
    type Error = MittyAntennaError;

    fn try_from(value: ElementRef<'_>) -> Result<Self, Self::Error> {
        let mut txt = value.text();

        let Some(title) = txt.next() else {
            return Err(MittyAntennaError::UpdateBoard("Title not found"));
        };

        let Some(_dots) = txt.next() else {
            return Err(MittyAntennaError::UpdateBoard(
                "Unused dots element not found",
            ));
        };

        let Some(body) = txt.next() else {
            return Err(MittyAntennaError::UpdateBoard("Body not found"));
        };

        return Ok(MittyUpdate {
            title: title.trim().to_string(),
            body: body.trim().to_string(),
        });
    }
}

async fn find_updates(body: &str) -> Result<Vec<MittyUpdate>, MittyAntennaError> {
    let html = scraper::Html::parse_document(body);

    let sel_table = scraper::Selector::parse("table").unwrap();
    let sel_td = scraper::Selector::parse("td").unwrap();

    let mut updates = vec![];

    for el in html.select(&sel_table) {
        let mut tds = el.select(&sel_td);

        while let Some(td) = tds.next() {
            if is_update_board(td) {
                break;
            }
        }

        while let Some(update) = tds.next() {
            if let Ok(mu) = MittyUpdate::try_from(update) {
                if mu.body == "[CORRUPTED DATA] [RESTARTING...]" {
                    return Ok(updates);
                }
                updates.push(mu);
            }
        }

        if updates.len() > 0 {
            return Ok(updates);
        }
    }

    return Err(MittyAntennaError::UpdateBoard("No posts found in HTML"));
}

fn is_update_board(td: ElementRef) -> bool {
    let mut txt = td.text();

    let Some(header) = txt.next() else {
        return false;
    };

    let Some(_dots) = txt.next() else {
        return false;
    };

    let Some(body) = txt.next() else {
        return false;
    };

    if header.trim() == "DD/MM HH:MM" && body.trim() == "Scroll to the right to read!" {
        return true;
    }
    return false;
}
