extern crate smtpd;
#[macro_use]
extern crate serde_derive;
extern crate envy;
extern crate serde;
extern crate slack_hook;

use std::error::Error;
use std::process::exit;

use slack_hook::{AttachmentBuilder, PayloadBuilder, Slack};
use smtpd::SmtpServer;

#[derive(Deserialize, Debug)]
struct Config {
  slack_hook: String,
}

#[derive(Debug, PartialEq, Default)]
struct Alert {
  message: String,
  status: Status,
}

#[derive(Debug, PartialEq)]
enum Status {
  Good,
  Bad,
  Info,
}

impl Default for Status {
  fn default() -> Status {
    Status::Good
  }
}

#[derive(PartialEq)]
enum ParseState {
  EmailHeader,
  EmailBody,
}

fn parse_haxproxy_alert(email: &str) -> Result<Alert, Box<Error>> {
  let mut alert = Alert::default();
  let mut state = ParseState::EmailHeader;
  let mut body = "";
  for line in email.lines() {
    if state == ParseState::EmailBody {
      body = line;
      break;
    }

    if line == "" {
      state = ParseState::EmailBody;
    }
  }

  alert.message = body.to_string();

  let parts: Vec<_> = body.split(',').collect();
  alert.status = if parts[0].contains("UP") {
    Status::Good
  } else if parts[0].contains("DOWN") {
    Status::Bad
  } else {
    Status::Info
  };

  Ok(alert)
}

fn main() {
  let cfg = match envy::from_env::<Config>() {
    Ok(cfg) => cfg,
    Err(err) => {
      match err {
        envy::Error::MissingValue(v) => {
          eprintln!("Missing ENV var: {:#?}", v.to_uppercase());
        }
        envy::Error::Custom(v) => {
          eprintln!("{}", v);
        }
      };

      exit(1);
    }
  };

  let slack = Slack::new(cfg.slack_hook.as_str()).unwrap();

  let mut smtp = SmtpServer::new();

  smtp.start_listener_thread("0.0.0.0:8025").unwrap();

  for mail in smtp {
    let alert = match parse_haxproxy_alert(&mail.message_body) {
      Ok(a) => a,
      Err(e) => {
        eprintln!("{}", e);
        continue;
      }
    };

    let mut lines = alert
      .message
      .split(|c| c == '.' || c == ',')
      .map(|x| x.trim());
    let message = lines.next().unwrap();
    let lines: Vec<_> = lines.collect();

    if let Err(e) = slack.send(&PayloadBuilder::new()
      .text(message)
      .username("haproxy-alert")
      .attachments(vec![
        AttachmentBuilder::new("")
          .text(lines.join("\n"))
          .color(if alert.status == Status::Good {
            "good"
          } else if alert.status == Status::Bad {
            "danger"
          } else {
            "warning"
          })
          .thumb_url(if alert.status == Status::Good {
            "https://png.icons8.com/color/75/000000/good-quality.png"
          } else if alert.status == Status::Bad {
            "https://png.icons8.com/color/75/000000/poor-quality.png"
          } else {
            "https://png.icons8.com/color/75/000000/info.png"
          })
          .build()
          .unwrap(),
      ])
      .build()
      .unwrap())
    {
      eprintln!("could not notify slack: {}", e);
    }
  }
}

#[cfg(test)]
mod test {
  use super::*;

  const DATA: &str = r#"From: haproxy@blockexplorer.com
To: devs@blockexplorer.com
Date: Tue, 01 May 2018 01:29:07 +0000 (UTC)
Subject: [HAproxy Alert] Server my-backend/mysrv is DOWN, reason: Layer4 timeout, check duration: 2002ms. 0 active and 0 backup servers left. 0 sessions active, 0 requeued, 0 remaining in queue

Server my-backend/mysrv is DOWN, reason: Layer4 timeout, check duration: 2002ms. 0 active and 0 backup servers left. 0 sessions active, 0 requeued, 0 remaining in queue
"#;

  #[test]
  fn test_parse_haproxy_alert() {
    // let res = parse_haxproxy_alert(DATA);
    // assert!(!res.is_err(), "{}", res.unwrap_err());
    // assert_eq!(res.unwrap(), Alert::default());
  }
}
