extern crate smtpd;
#[macro_use]
extern crate serde_derive;
extern crate envy;
extern crate regex;
extern crate serde;
extern crate slack_hook;
#[macro_use]
extern crate lazy_static;

use std::error::Error;
use std::process::exit;

use regex::Regex;
use slack_hook::{AttachmentBuilder, Field, PayloadBuilder, Slack};
use smtpd::SmtpServer;

lazy_static! {
  static ref RE_DOWN: Regex = Regex::new(r"(Server.*?), reason: (.*?), check duration: (.*?). (\d+) active and (\d+) backup servers left. (\d+) sessions active, (\d+) requeued, (\d+) remaining in queue").unwrap();
  static ref RE_UP: Regex = Regex::new(r"(Server.*?), reason: (.*?), code: (\d+), info: (.*?), check duration: (.*?). (\d+) active and (\d+) backup servers online. (\d+) sessions requeued, (\d+) total in queue").unwrap();
}

#[derive(Deserialize, Debug)]
struct Config {
  slack_hook: String,
}

#[derive(Debug, PartialEq, Default)]
struct Alert {
  message: String,
  reason: String,
  status: Status,
  keyvalue: Vec<(String, String)>,
}

#[derive(Debug, PartialEq)]
enum Status {
  Up,
  Down,
}

impl Default for Status {
  fn default() -> Status {
    Status::Up
  }
}

#[derive(PartialEq)]
enum ParseState {
  EmailHeader,
  EmailBody,
  // Message,
  // KeyValue,
  // ServersLeft,
}

fn capitalize(s: &str) -> String {
  let mut c = s.chars();
  match c.next() {
    None => String::new(),
    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
  }
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

  if let Some(captures) = RE_UP.captures(body) {
    alert.message = captures[1].to_string();
    alert.reason = captures[2].to_string();
    alert.status = Status::Up;
    alert
      .keyvalue
      .push(("code".to_string(), captures[3].to_string()));
    alert
      .keyvalue
      .push(("info".to_string(), captures[4].to_string()));
    alert
      .keyvalue
      .push(("check duration".to_string(), captures[5].to_string()));
    alert
      .keyvalue
      .push(("active servers online".to_string(), captures[6].to_string()));
    alert
      .keyvalue
      .push(("backup servers online".to_string(), captures[7].to_string()));
    alert
      .keyvalue
      .push(("sessions requeued".to_string(), captures[8].to_string()));
    alert
      .keyvalue
      .push(("total in queue".to_string(), captures[8].to_string()));
  } else if let Some(captures) = RE_DOWN.captures(body) {
    alert.message = captures[1].to_string();
    alert.reason = captures[2].to_string();
    alert.status = Status::Down;
    alert
      .keyvalue
      .push(("check duration".to_string(), captures[3].to_string()));
    alert
      .keyvalue
      .push(("active servers left".to_string(), captures[4].to_string()));
    alert
      .keyvalue
      .push(("backup servers left".to_string(), captures[5].to_string()));
    alert
      .keyvalue
      .push(("sessions active".to_string(), captures[6].to_string()));
    alert
      .keyvalue
      .push(("requeued".to_string(), captures[7].to_string()));
    alert
      .keyvalue
      .push(("remaining in queue".to_string(), captures[8].to_string()));
  } else {
    return Err(format!("parse error: regex doesn't match: {}", body).into());
  }

  // let parts: Vec<&str> = body.split(|c| c == '.' || c == ',').collect();
  // if parts.len() != 7 {
  //   return Err(format!(
  //     "could not parse alert. parts.len() expected: 7, got: {}",
  //     parts.len()
  //   ));
  // }

  // alert.message = parts[0].to_owned();

  // let mut buf = vec![];
  // state = ParseState::Message;
  // for c in body.chars() {
  //   match c {
  //     ',' => {
  //       match state {
  //         ParseState::Message => {
  //           state = ParseState::KeyValue;
  //           alert.message = buf.iter().collect();
  //           buf.clear();
  //         }
  //         ParseState::KeyValue => {
  //           let index = buf
  //             .iter()
  //             .position(|&x| x == ':')
  //             .ok_or("could not find ':' in key/value")?;
  //           alert.map.insert(
  //             // Does this create two allocations?
  //             buf[..index].iter().collect::<String>().trim().to_string(),
  //             buf[index + 1..]
  //               .iter()
  //               .collect::<String>()
  //               .trim()
  //               .to_string(),
  //           );
  //         }
  //         _ => {
  //           return Err("parse error".into());
  //         }
  //       };
  //     }
  //     '.' => {
  //       match state {
  //         ParseState::KeyValue => state = ParseState::ServersLeft,
  //         ParseState::ServersLeft => {
  //           let parts: Vec<_> = buf.split(|&x| x == ' ').collect();
  //           if parts.len() != 7 {
  //             return Err(format!(
  //               "could not parse servers left. parts.len() expected: 7, got: {}",
  //               parts.len()
  //             ));
  //           }

  //           //alert.map.insert(parts[0],
  //         }
  //         _ => {
  //           return Err("parse error".into());
  //         }
  //       };
  //     }
  //     _ => buf.push(c),
  //   }
  // }

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

    match slack.send(&PayloadBuilder::new()
      .text(format!("{}: {}", alert.message, alert.reason))
      .username("haproxy-alert")
      .attachments(vec![
        AttachmentBuilder::new("")
          .color(if alert.status == Status::Up {
            "good" // "#81f292"
          } else {
            "danger" // "#f38181"
          })
          .fields(
            alert
              .keyvalue
              .iter()
              .map(|ref x| Field::new(capitalize(&x.0), capitalize(&x.1), Some(true)))
              .collect(),
          )
          .thumb_url(if alert.status == Status::Up {
            "https://png.icons8.com/color/96/000000/good-quality.png"
          } else {
            "https://png.icons8.com/color/96/000000/poor-quality.png"
          })
          .build()
          .unwrap(),
      ])
      .build()
      .unwrap())
    {
      Err(e) => eprintln!("could not notify slack: {}", e),
      Ok(_) => (),
    };
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
