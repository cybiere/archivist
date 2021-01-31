use regex::Regex;
use std::collections::{VecDeque,HashMap};
use std::thread::{self,JoinHandle};
use std::time::Duration;
use std::sync::mpsc;

const MAX_THREADS : usize = 32;

fn process(target: &str, root: &str, links_regex: Regex, cleanup_regex: Regex, ltx: mpsc::Sender<String>)-> Result<(),Box<dyn std::error::Error>>{
    let body: String = ureq::get(&target)
        .call()?
        .into_string()?;

    let mut count = 0;
    for capture in links_regex.captures_iter(&body){
        let mut link = String::from(capture.name("link").unwrap().as_str());
        if link.starts_with("/"){
            link = format!("{}{}",&root,&link);
        }
        if ! link.starts_with(root){
            continue
        }
        count += 1;
        ltx.send(link).unwrap();
    }
    println!("> Found {} from {}", count, target);
    Ok(())
}

fn main() -> Result<(),Box<dyn std::error::Error>> {

    let mut target = std::env::args().nth(1).expect("no target given");
    let domain_regex = Regex::new(r"^(?P<root>([a-zA-Z]+://)?(?P<dom>.+?))(/.*)?$").unwrap(); 

    let caps = domain_regex.captures(&target).unwrap();
    let domain = String::from(caps.name("dom").unwrap().as_str());
    let root = String::from(caps.name("root").unwrap().as_str());

    let cleanup_regex : Regex = Regex::new(r"(?s)(<!--.*?-->)").unwrap();
    let links_regex : Regex = Regex::new(r##"(href|src)=('|")(?P<link>.+?)('|")"##).unwrap();

    println!("{}",root);

    if ! target.starts_with("http"){
        target = format!("http://{}",&target);
    }
    if target == root{
        target.push_str("/");
    }
    println!("{}",target);

    let mut todo = VecDeque::new();
    todo.push_back(target);
    let mut done = Vec::new();

    let mut doing = HashMap::<String,JoinHandle<()>>::new();
    let (links_tx, links_rx) = mpsc::channel::<String>();
    let (threads_tx, threads_rx) = mpsc::channel::<String>();

    while ! ( todo.is_empty() && doing.is_empty() ){
        for received in links_rx.try_iter() {
            if ! (todo.contains(&received) || done.contains(&received) || doing.contains_key(&received) ){
                todo.push_back(received);
            }
        }
        for received in threads_rx.try_iter() {
            let handle = doing.remove(&received).unwrap();
            handle.join();
            println!("Done {}",&received);
            done.push(received);
        }
        if ! todo.is_empty() {
            if doing.len() == MAX_THREADS{
                continue;
            }
            let next = todo.pop_front().unwrap();
            let ltx = mpsc::Sender::clone(&links_tx);
            let ttx = mpsc::Sender::clone(&threads_tx);
            let lroot = root.clone();
            let llinks_regex = links_regex.clone();
            let lcleanup_regex = cleanup_regex.clone();
            doing.insert(next.clone(), thread::spawn(move || {
                process(&next, &lroot, llinks_regex, lcleanup_regex, ltx);
                ttx.send(next).unwrap();
            }));
        }
    }

    Ok(())
}
