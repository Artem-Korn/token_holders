CREATE TABLE IF NOT EXISTS token (
    token_id INT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    contract_addr BYTEA UNIQUE NOT NULL,
    last_checked_block BIGINT NOT NULL,
    symbol VARCHAR(10) NOT NULL,
    decimals SMALLINT NOT NULL
);

CREATE TABLE IF NOT EXISTS holder (
    holder_id INT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    holder_addr BYTEA UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS balance (
    holder_id INT NOT NULL,
    token_id INT NOT NULL,
    amount NUMERIC(78, 0) NOT NULL,
    PRIMARY KEY (holder_id, token_id),
    CONSTRAINT fk_holder
      FOREIGN KEY(holder_id) 
      REFERENCES holder(holder_id),
    CONSTRAINT fk_token
      FOREIGN KEY(token_id) 
      REFERENCES token(token_id)
);

CREATE INDEX IF NOT EXISTS idx_token_id ON balance (token_id);