use crate::{
    config,
    jmap::{EmailKeyword, Id},
    local,
    remote::{AvailableMailboxRoles, Email, Mailbox, Mailboxes},
};
use log::debug;
use serde_json::Value;

use std::{collections::HashMap, collections::HashSet, path::PathBuf};

pub struct Create<'a> {
    pub path: PathBuf,
    pub mailboxes: Vec<&'a Id>,
    pub keywords: Vec<EmailKeyword>,
}

pub struct StateDelta<'a> {
    pub updates: HashMap<&'a Id, HashMap<&'a str, Value>>,
    pub creates: Vec<Create<'a>>,
}

pub fn compute_state_delta<'a>(
    mailboxes: &'a Mailboxes,
    tags_config: &'a config::Tags,
    remote_emails: &'a HashMap<Id, Email>,
    local_emails: &'a HashMap<Id, local::Email>,
) -> StateDelta<'a> {
    // Updates are a list of patches
    let mut updates: HashMap<&Id, HashMap<&str, Value>> = HashMap::new();
    // Creation events will be EmailImport statements
    let mut creates: Vec<Create> = Vec::new();

    // Build patches.
    local_emails.iter().for_each(|(id, local_email)| {
        fn as_value(b: bool) -> Value {
            if b { Value::Bool(true) } else { Value::Null }
        }

        // A list of all remote mailbox IDs, corresponding to local tags, which the email should possess
        let mut new_mailboxes: Vec<&Id> = mailboxes
            .mailboxes_by_id
            .values()
            .filter_map(|v| {
                if local_email.tags.contains(&v.tag) {
                    return Some(&v.id);
                } else {
                    return None;
                }
            })
            .collect();

        // If no mailboxes were found, assign to Archive.
        if new_mailboxes.is_empty() {
            new_mailboxes.push(&mailboxes.archive_id);
        }

        match remote_emails.get(id) {
            Some(remote_email) => {
                let mut patch = HashMap::new();

                // Keywords.
                patch.insert(
                    "keywords/$draft",
                    as_value(local_email.tags.contains("draft")),
                );
                patch.insert(
                    "keywords/$seen",
                    as_value(!local_email.tags.contains("unread")),
                );
                patch.insert(
                    "keywords/$flagged",
                    as_value(local_email.tags.contains("flagged")),
                );
                patch.insert(
                    "keywords/$answered",
                    as_value(local_email.tags.contains("replied")),
                );
                patch.insert(
                    "keywords/$forwarded",
                    as_value(local_email.tags.contains("passed")),
                );
                if mailboxes.roles.spam.is_none() && !tags_config.spam.is_empty() {
                    let spam = local_email.tags.contains(&tags_config.spam);
                    patch.insert("keywords/$junk", as_value(spam));
                    patch.insert("keywords/$notjunk", as_value(!spam));
                }
                if !tags_config.phishing.is_empty() {
                    patch.insert(
                        "keywords/$phishing",
                        as_value(local_email.tags.contains(&tags_config.phishing)),
                    );
                }

                new_mailboxes.extend(
                    remote_email
                        .mailbox_ids
                        .iter()
                        .filter(|x| mailboxes.ignored_ids.contains(x)),
                );

                patch.insert(
                    "mailboxIds",
                    Value::Object(
                        new_mailboxes
                            .into_iter()
                            .map(|x| (x.0.clone(), Value::Bool(true)))
                            .collect::<serde_json::Map<String, Value>>(),
                    ),
                );

                updates.insert(id, patch);
            }
            None => {
                let mut keywords: Vec<EmailKeyword> = Vec::new();
                if local_email.tags.contains("draft") {
                    keywords.push(EmailKeyword::Draft);
                }

                if !local_email.tags.contains("unread") {
                    keywords.push(EmailKeyword::Seen);
                }

                if local_email.tags.contains("flagged") {
                    keywords.push(EmailKeyword::Flagged);
                }

                if local_email.tags.contains("replied") {
                    keywords.push(EmailKeyword::Answered);
                }

                if local_email.tags.contains("passed") {
                    keywords.push(EmailKeyword::Forwarded);
                }

                creates.push(Create {
                    path: local_email.path.clone(),
                    mailboxes: new_mailboxes,
                    keywords,
                });
            }
        }
    });
    debug!("Built patch for remote: {:?}", updates);

    return StateDelta {
        creates: creates,
        updates: updates,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    struct Fixture {
        mailboxes: Mailboxes,
        tags: config::Tags,
        remote_emails: HashMap<Id, Email>,
        local_emails: HashMap<Id, local::Email>,
    }

    impl Fixture {
        fn new() -> Fixture {
            Fixture {
                mailboxes: Mailboxes {
                    archive_id: Id("Archive".to_string()),
                    mailboxes_by_id: HashMap::<Id, Mailbox>::new(),
                    ids_by_tag: HashMap::<String, Id>::new(),
                    ignored_ids: HashSet::<Id>::new(),
                    roles: AvailableMailboxRoles::default(),
                },
                tags: config::Tags::default(),
                remote_emails: HashMap::<Id, Email>::new(),
                local_emails: HashMap::<Id, local::Email>::new(),
            }
        }
    }

    #[test]
    fn noop_is_noop() {
        let fixture = Fixture::new();

        let result = compute_state_delta(
            &fixture.mailboxes,
            &fixture.tags,
            &fixture.remote_emails,
            &fixture.local_emails,
        );

        assert!(result.updates.len() == 0);
        assert!(result.creates.len() == 0);
    }

    #[test]
    fn new_remote_email() {
        let mut fixture = Fixture::new();
        fixture.remote_emails.insert(
            Id("1".to_owned()),
            Email {
                id: Id("1".to_owned()),
                blob_id: Id("ffff".to_owned()),
                keywords: HashSet::<EmailKeyword>::new(),
                mailbox_ids: HashSet::<Id>::new(),
                tags: HashSet::<String>::new(),
            },
        );

        let result = compute_state_delta(
            &fixture.mailboxes,
            &fixture.tags,
            &fixture.remote_emails,
            &fixture.local_emails,
        );

        // TODO The delta engine should handle new remote mail
        assert!(result.updates.len() == 0);
        assert!(result.creates.len() == 0);
    }

    #[test]
    fn new_local_email() {
        let mut fixture = Fixture::new();
        fixture.local_emails.insert(
            Id("1".to_owned()),
            local::Email {
                id: Id("1".to_owned()),
                blob_id: Id("ffff".to_owned()),
                message_id: "l1".to_owned(),
                path: PathBuf::from("/home/user/.mail"),
                tags: HashSet::<String>::from([]),
            },
        );

        let result = compute_state_delta(
            &fixture.mailboxes,
            &fixture.tags,
            &fixture.remote_emails,
            &fixture.local_emails,
        );

        assert!(result.updates.len() == 0);
        assert!(result.creates.len() == 1);

        let create = &result.creates[0];
        assert_eq!(Path::new("/home/user/.mail"), create.path);

        // TODO: How do we propagate NotMuch tags?
        //assert_eq!(vec!(&Id("hello".to_owned())) , create.mailboxes);
        //assert_eq!(HashSet::<String>::from(["hello".to_owned()]), create.keywords);
    }

    #[test]
    fn updated_email() {
        let mut fixture = Fixture::new();
        fixture.remote_emails.insert(
            Id("1".to_owned()),
            Email {
                id: Id("1".to_owned()),
                blob_id: Id("ffff".to_owned()),
                keywords: HashSet::<EmailKeyword>::new(),
                mailbox_ids: HashSet::<Id>::new(),
                tags: HashSet::<String>::from(["hello".to_owned()]),
            },
        );
        fixture.local_emails.insert(
            Id("1".to_owned()),
            local::Email {
                id: Id("1".to_owned()),
                blob_id: Id("ffff".to_owned()),
                message_id: "l1".to_owned(),
                path: PathBuf::new(),
                tags: HashSet::<String>::new(),
            },
        );

        let result = compute_state_delta(
            &fixture.mailboxes,
            &fixture.tags,
            &fixture.remote_emails,
            &fixture.local_emails,
        );

        assert!(result.updates.len() == 1);
        assert!(result.creates.len() == 0);
    }
}
