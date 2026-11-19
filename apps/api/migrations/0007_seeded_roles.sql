-- The editable Roles a Village starts from; see `role::Permission` for the catalog.
INSERT INTO roles (name, permissions) VALUES
    ('Operator', '{village.manage,content.write,content.publish,complaint.read,complaint.handle}'),
    ('Kepala Desa', '{village.manage,content.write,content.publish,complaint.read,complaint.handle,audit.read}'),
    ('Kontributor Web', '{content.write,content.publish}');
