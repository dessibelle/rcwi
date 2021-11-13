use std::{
    sync::{mpsc::Receiver, Arc, Mutex},
    time::Duration,
};

use aws_sdk_cloudwatchlogs::Client;
use indicium::simple::{Indexable, SearchIndex};
use log::{error, info};

use crate::{log_groups::filter_log_groups, App};

pub(crate) enum AwsReq {
    ListLogGroups,
    RunQuery,
}

struct MyString {
    s: String,
}

impl From<&str> for MyString {
    fn from(st: &str) -> Self {
        MyString { s: st.to_string() }
    }
}
impl Indexable for MyString {
    fn strings(&self) -> Vec<String> {
        vec![self.s.clone()]
    }
}

pub(crate) fn run(app: Arc<Mutex<App>>, rx: Receiver<AwsReq>) {
    let basic_rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    basic_rt.block_on(async {
        let shared_config = aws_config::load_from_env().await;
        let client = Client::new(&shared_config);
        loop {
            if let Ok(req) = rx.recv() {
                match req {
                    AwsReq::ListLogGroups => {
                        let mut res = client.describe_log_groups().send().await.unwrap();
                        {
                            let names: Vec<String> = res
                                .log_groups
                                .unwrap_or(vec![])
                                .iter()
                                .map(|z| z.log_group_name.as_ref().unwrap().clone())
                                .collect();
                            let mut app_ = app.lock().unwrap();
                            app_.log_group_search_index = SearchIndex::default();
                            names
                                .iter()
                                .map(|x| MyString::from(x.as_str()))
                                .enumerate()
                                .for_each(|(index, element)| {
                                    app_.log_group_search_index.insert(&index, &element)
                                });
                            app_.log_groups = names;
                            filter_log_groups(&mut app_);
                        }
                        loop {
                            if res.next_token.is_none() {
                                break;
                            }
                            res = client
                                .describe_log_groups()
                                .next_token(res.next_token.as_ref().unwrap())
                                .send()
                                .await
                                .unwrap();
                            let names: Vec<String> = res
                                .log_groups
                                .unwrap_or(vec![])
                                .iter()
                                .map(|z| z.log_group_name.as_ref().unwrap().clone())
                                .collect();

                            {
                                let mut app_ = app.lock().unwrap();
                                names
                                    .iter()
                                    .map(|x| MyString::from(x.as_str()))
                                    .enumerate()
                                    .for_each(|(index, element)| {
                                        app_.log_group_search_index.insert(&index, &element)
                                    });
                                app_.log_groups.extend(names);
                                filter_log_groups(&mut app_);
                            }
                        }
                    }
                    AwsReq::RunQuery => {
                        let (log_groups, query_string) = {
                            let app_ = app.lock().unwrap();
                            let log_groups = app_.selected_log_groups.clone();
                            (log_groups, app_.query.clone())
                        };
                        let res = client
                            .start_query()
                            .set_log_group_names(Some(log_groups))
                            .query_string(query_string)
                            .start_time(0i64)
                            .end_time(1636811010i64)
                            .send()
                            .await;
                        match res {
                            Ok(res) => {

                                if let Some(query_id) = res.query_id {
                                    let mut res;
                                    loop {
                                        res = client.get_query_results().query_id(query_id.clone()).send().await.unwrap();
                                        match res.status {
                                            Some(x) if x != aws_sdk_cloudwatchlogs::model::QueryStatus::Running => break,
                                            _ => {
                                                info!("query: {:?}", res);
                                                if let Some(results) = res.results {
                                                    let mut app_ = app.lock().unwrap();
                                                    app_.results = results.iter().map(|x| x.iter().map(|y| format!("{:?}", y)).collect::<Vec<_>>().join(", ")).collect();
                                                }
                                            },
                                        }
                                        tokio::time::sleep(Duration::from_millis(500)).await;
                                    }
                                    if let Some(results) = res.results {
                                        let mut app_ = app.lock().unwrap();
                                        app_.results = results.iter().map(|x| x.iter().map(|y| format!("{:?}", y)).collect::<Vec<_>>().join(", ")).collect();
                                    }
                                }
                            },
                            Err(e) => error!("{:?}", e),
                        }
                    }

                }
            }
        }
    });
}