INSERT INTO permissions (key) VALUES
    ('sandbox:create'),
    ('sandbox:destroy'),
    ('sandbox:exec'),
    ('user:invite'),
    ('user:remove')
ON CONFLICT (key) DO NOTHING;