extern crate chrono;
extern crate egg_mode;
#[macro_use]
extern crate lazy_static;
extern crate tokio;
extern crate tokio_core;

use std::env;
use std::io::*;
use std::iter::Iterator;
use std::rc::*;
use std::time;

use chrono::prelude::*;
use egg_mode::*;
use tokio::prelude::Future;
use tokio::prelude::Stream;
use tokio_core::reactor::Core;
use tokio_core::reactor::Interval;

const TWEET_STR: &'static str = "そんなことしてずに研究しろよ！";

lazy_static! {
    static ref VICTIM_SCREEN_NAME: String = get_env("VICTIM_SCREEN_NAME");
}

struct TwitterCredentials {
    api_key: String,
    api_secret: String,
    access_token: String,
    access_secret: String,
}

impl TwitterCredentials {
    fn init_from_env() -> TwitterCredentials {
        TwitterCredentials {
            api_key: get_env("TWITTER_API_KEY"),
            api_secret: get_env("TWITTER_API_SECRET"),
            access_token: get_env("TWITTER_ACCESS_TOKEN"),
            access_secret: get_env("TWITTER_ACCESS_SECRET"),
        }
    }
}

fn get_env(key: &str) -> String {
    env::var(key).unwrap_or_else(|_| {
        eprintln!("You must specify {}", key);
        eprintln!("Quitting");
        std::process::exit(1);
    })
}

fn main() {
    let twitter_credentials = TwitterCredentials::init_from_env();
    let token = Token::Access {
        consumer: KeyPair::new(twitter_credentials.api_key, twitter_credentials.api_secret),
        access: KeyPair::new(twitter_credentials.access_token, twitter_credentials.access_secret)
    };

    let mut core = Core::new().unwrap();
    let handle = Rc::new(core.handle());
    let user = verify_tokens(&token, &handle);
    let user2 = core.run(user).unwrap();
    println!("{:?}", user2);
    let interval = Interval::new(time::Duration::from_secs(3), &handle.clone()).unwrap();
    let l = interval
        .for_each(move |_| {
            let timeline = egg_mode::tweet::user_timeline(
                &*VICTIM_SCREEN_NAME,
                true,
                false,
                &token.clone(),
                &handle.clone(),
            );
            /* https://bit.ly/2HCrd9U */
            let handle_ptr = handle.clone();
            let token_ptr = token.clone();
            let timeline_future = timeline
                .start()
                .then(move |r| {
                    let (_timeline, feed) = r.unwrap();
                    let recent_tweets = feed.into_iter()
                        .filter(move |tweet| {
                            let threshold = Utc::now() - chrono::Duration::minutes(5);
                            tweet.created_at > threshold
                        })
                        .collect::<Vec<egg_mode::Response<egg_mode::tweet::Tweet>>>();

                    println!("tweets in the past 5 minutes: {}", recent_tweets.len());
                    recent_tweets.iter().for_each(|tweet| {
                        println!(
                            "<@{}> {}",
                            tweet.user.as_ref().unwrap().screen_name,
                            tweet.text
                        );
                    });
                    if recent_tweets.len() >= 10 {
                        let latest_tweet = &recent_tweets[0];
                        let victim_name = &latest_tweet.user.as_ref().unwrap().screen_name;
                        let draft =
                            tweet::DraftTweet::new(format!("@{} {}", victim_name, TWEET_STR))
                                .in_reply_to(latest_tweet.id);
                        handle_ptr.spawn(draft.send(&token_ptr, &handle_ptr).then(|r| {
                            let _res = r.map(|t| {
                                println!("Replied to {:?}", t.in_reply_to_screen_name);
                            }).map_err(|e| {
                                println!("Shit happened: {:?}", e);
                                e
                            });
                            Ok(())
                        }));
                    }
                    Ok::<(), Error>(())
                })
                .map_err(|err| {
                    println!("Error: {}", err);
                });
            handle.spawn(timeline_future);
            Ok(())
        })
        .map_err(|err| println!("Shit happened: {}", err));

    core.run(l).unwrap_or_else(|_| {
        println!("Main loop failed");
    });
}
