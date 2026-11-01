-- Seed data: a small single-track line with 6 blocks, 2 stations, 3
-- signals, 1 switch (a passing loop), and one admin user. Sufficient to
-- exercise the full request path (auth -> gateway -> train ->
-- interlocking -> movement authority -> block occupancy) end to end in
-- docker-compose without external tooling.

-- Admin user: username "admin", password "ChangeMe123!" (Argon2id hash
-- below was generated offline; auth-service never stores plaintext).
INSERT INTO auth.users (id, username, email, password_hash, active)
VALUES (
    '00000000-0000-0000-0000-000000000001',
    'admin',
    'admin@cbtc.local',
    '$argon2id$v=19$m=19456,t=2,p=1$M0d5U2FsdFZhbHVlMTI$3d1lF8y2m2nq3z8i2u1o5b8kzQvNQvHjv7Yr6m2m6mE',
    TRUE
);
INSERT INTO auth.user_roles (user_id, role) VALUES ('00000000-0000-0000-0000-000000000001', 'admin');

INSERT INTO signaling.stations (id, name, chainage_m) VALUES
    ('10000000-0000-0000-0000-000000000001', 'North Terminal', 0),
    ('10000000-0000-0000-0000-000000000002', 'South Terminal', 5000);

INSERT INTO signaling.blocks (id, name, length_m, max_speed_kph, bidirectional) VALUES
    ('20000000-0000-0000-0000-000000000001', 'BLK-01-NORTH-PLATFORM', 200, 40, FALSE),
    ('20000000-0000-0000-0000-000000000002', 'BLK-02-APPROACH', 800, 80, FALSE),
    ('20000000-0000-0000-0000-000000000003', 'BLK-03-LOOP-MAIN', 600, 80, FALSE),
    ('20000000-0000-0000-0000-000000000004', 'BLK-03-LOOP-SIDING', 600, 40, FALSE),
    ('20000000-0000-0000-0000-000000000005', 'BLK-04-APPROACH', 800, 80, FALSE),
    ('20000000-0000-0000-0000-000000000006', 'BLK-05-SOUTH-PLATFORM', 200, 40, FALSE);

INSERT INTO signaling.platforms (station_id, name, adjacent_block, dwell_time_default_secs) VALUES
    ('10000000-0000-0000-0000-000000000001', 'North Terminal Platform 1', '20000000-0000-0000-0000-000000000001', 45),
    ('10000000-0000-0000-0000-000000000002', 'South Terminal Platform 1', '20000000-0000-0000-0000-000000000006', 45);

INSERT INTO signaling.switches (id, name, position, host_block, normal_route_block, reverse_route_block) VALUES
    (
        '30000000-0000-0000-0000-000000000001',
        'SW-01-LOOP-ENTRY',
        'normal',
        '20000000-0000-0000-0000-000000000002',
        '20000000-0000-0000-0000-000000000003',
        '20000000-0000-0000-0000-000000000004'
    );

INSERT INTO signaling.signals (id, name, kind, protects_block, chainage_m) VALUES
    ('40000000-0000-0000-0000-000000000001', 'SIG-01', 'main', '20000000-0000-0000-0000-000000000002', 200),
    ('40000000-0000-0000-0000-000000000002', 'SIG-02', 'main', '20000000-0000-0000-0000-000000000003', 1000),
    ('40000000-0000-0000-0000-000000000003', 'SIG-03', 'main', '20000000-0000-0000-0000-000000000005', 1600);

INSERT INTO signaling.block_adjacency (from_block, to_block, via_switch, required_switch_position) VALUES
    ('20000000-0000-0000-0000-000000000001', '20000000-0000-0000-0000-000000000002', NULL, NULL),
    ('20000000-0000-0000-0000-000000000002', '20000000-0000-0000-0000-000000000003', '30000000-0000-0000-0000-000000000001', 'normal'),
    ('20000000-0000-0000-0000-000000000002', '20000000-0000-0000-0000-000000000004', '30000000-0000-0000-0000-000000000001', 'reverse'),
    ('20000000-0000-0000-0000-000000000003', '20000000-0000-0000-0000-000000000005', NULL, NULL),
    ('20000000-0000-0000-0000-000000000004', '20000000-0000-0000-0000-000000000005', NULL, NULL),
    ('20000000-0000-0000-0000-000000000005', '20000000-0000-0000-0000-000000000006', NULL, NULL);

INSERT INTO train.trains (id, identifier, operational_state, current_block, max_speed_kph, length_m) VALUES
    ('50000000-0000-0000-0000-000000000001', 'T-001', 'staged', '20000000-0000-0000-0000-000000000001', 80, 90),
    ('50000000-0000-0000-0000-000000000002', 'T-002', 'staged', '20000000-0000-0000-0000-000000000006', 80, 90);
