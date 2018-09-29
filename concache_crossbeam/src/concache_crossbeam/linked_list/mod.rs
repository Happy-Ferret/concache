pub mod node;

use self::node::Node;
use crossbeam::epoch::{self, Atomic, Owned};

use std::sync::atomic::Ordering;
use std::fmt;

pub (super) struct LinkedList {
    first: Atomic<Node>,
}

impl LinkedList {
    pub (super) fn new () -> Self {
        LinkedList {
            first: Atomic::null()
        }
    }

    pub (super) fn insert (&self, kv : (usize, usize)) -> bool {
        let guard = epoch::pin();

        let mut node = &self.first;
        loop {
            let l = node.load(Ordering::Relaxed, &guard);
            match l {
                Some(k) => {
                    let mut raw = k.as_raw();
                    let mut cur = unsafe { &*raw };
                    if cur.kv.0 == kv.0 && cur.active.load(Ordering::Relaxed) {
                        let mut change = cur.kv.1.lock().unwrap();
                        *change = kv.1;
                        return false;
                    }
                    node = &k.next;

                    // key does not exist
                    if cur.next.load(Ordering::Relaxed, &guard).is_none() {
                        let mut ins = Owned::new(Node::new(kv.0, kv.1));
                        ins.prev.store_shared(l, Ordering::Relaxed);
                        cur.next.store_and_ref(ins, Ordering::Relaxed, &guard);
                    }
                },
                None => {
                    break;
                }
            };
        }
        return true;
    }

    pub (super) fn get (&self, key : usize) -> Option<usize> {
        let guard = epoch::pin();

        let mut node = &self.first;
        loop {
            match node.load(Ordering::Relaxed, &guard) {
                Some(k) => {
                    let mut raw = k.as_raw();
                    let mut cur = unsafe { &*raw };
                    if cur.kv.0 == key && cur.active.load(Ordering::Relaxed) {
                        let value = cur.kv.1.lock().unwrap();
                        return Some(*value);
                    }
                    node = &k.next;
                },
                None => {
                    return None;
                }
            };
        }

    }

    pub (super) fn remove (&self, key : usize) -> bool {
        let guard = epoch::pin();

        let mut node = &self.first;
        loop {
            match node.load(Ordering::Relaxed, &guard) {
                Some(k) => {
                    let mut raw = k.as_raw();
                    let mut cur = unsafe { &*raw };
                    if cur.kv.0 == key && cur.active.load(Ordering::Relaxed) {
                        cur.active.store(false, Ordering::SeqCst);

                        let next = k.next.load(Ordering::Relaxed, &guard);
                        let prev = k.prev.load(Ordering::Relaxed, &guard);

                        node.cas_shared(Some(k), next, Ordering::Release);

                        let mut new_node = match node.load(Ordering::Relaxed, &guard) {
                            Some(k) => k,
                            None => {
                                continue;
                            }
                        };
                        let mut new_node_raw_cur = unsafe { &*new_node.as_raw() };

                        if new_node_raw_cur.prev.cas_shared(Some(k), prev, Ordering::Release) {
                            unsafe { guard.unlinked(k) };
                            return true;
                        }
                    }
                    node = &k.next;
                },
                None => {
                    // the node with key key didn't exist
                    return false;
                }
            };
        }
    }
}

impl fmt::Display for LinkedList {
    fn fmt (&self, f : &mut fmt::Formatter) -> fmt::Result {
        let guard = epoch::pin();

        let mut ret = String::new();
        let mut node = &self.first;
        loop {
            match node.load(Ordering::Relaxed, &guard) {
                Some(k) => {
                    let mut raw = k.as_raw();
                    let mut cur = unsafe { &*raw };
                    if cur.active.load(Ordering::Relaxed) {
                        let key = cur.kv.0;
                        println!("Taking lock for value");
                        let value = cur.kv.1.lock().unwrap();
                        println!("Took lock for value");

                        ret.push_str("(");
                        ret.push_str(&key.to_string());
                        ret.push_str(", ");
                        ret.push_str(&value.to_string());
                        ret.push_str("), ");

                        println!("Releasing lock for value");
                    }
                    node = &k.next;
                },
                None => {
                    break;
                }
            };
        }

        write!(f, "{}", ret)
    }
}
