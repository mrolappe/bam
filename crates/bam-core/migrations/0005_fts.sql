-- Full-text index over package.description and readme text. Contentless
-- (content=''): the FTS b-tree is built from the values passed at insert
-- time, but no original text is retained for retrieval — description and
-- readme text already live in `package`/`landing_readme`, so nothing here
-- needs to duplicate them for storage, only for search. Populated and kept
-- in sync exclusively by `store::fts::rebuild_fts` (explicit rebuild, no
-- triggers): the normalized `package` layer is itself bulk-rebuilt from
-- landing data (P1.6), which a trigger-only design would silently desync.
CREATE VIRTUAL TABLE package_fts USING fts5(description, readme_text, content='');
