UPDATE auth.users
SET password_hash = '$argon2id$v=19$m=19456,t=2,p=1$k/FfFK2iumz9KegsygIdlw$wtLg7u858cJ309noIhHSr9FW45dhxkdqFmo5uVYnE+A'
WHERE username = 'admin';
