#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_is_noop() {
	let mailboxes = Mailboxes {
	    archive_id: Id("Archive".to_string()),
	    mailboxes_by_id: HashMap::<Id, Mailbox>::new(),
	    ids_by_tag: HashMap::<String, Id>::new(),
	    ignored_ids: HashSet::<Id>::new(),
	    roles: AvailableMailboxRoles::default(),
	};
	let tags = config::Tags::default();
	let remote_emails = HashMap::<Id, Email>::new();
	let local_emails = HashMap::<Id, local::Email>::new();
	

	assert!(result.updates.len() == 0);
	assert!(result.creates.len() == 0);
    }
}
