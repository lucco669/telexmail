-- Version 25: the most an account's SMTP server takes in one message, in bytes, as its EHLO
-- answer says (SIZE, RFC 1870), read at each login to send. NULL: not said (or 0, "no fixed
-- limit"); compose then goes by the provider list, else 20 MB (sizes.rs). The limit is for the
-- message as sent: encoded, files and pictures in base64.
ALTER TABLE accounts ADD COLUMN smtp_max INTEGER;
