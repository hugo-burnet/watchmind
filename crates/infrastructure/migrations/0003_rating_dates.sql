ALTER TABLE ratings ADD COLUMN rated_at_unix INTEGER CHECK (rated_at_unix IS NULL OR rated_at_unix >= 0);
