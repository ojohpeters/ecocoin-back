-- Add migration script here
-- migrations/XXXXXXXXXX_init.sql

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE users (
  id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
  wallet_address TEXT UNIQUE NOT NULL,
  total_points INT DEFAULT 0,
  referrer_id UUID,
  referral_code UUID DEFAULT uuid_generate_v4()
);

CREATE TABLE tasks (
  id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
  name TEXT NOT NULL,
  points INT NOT NULL
);

CREATE TABLE completed_tasks (
  id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
  user_id UUID NOT NULL REFERENCES users(id),
  task_id UUID NOT NULL REFERENCES tasks(id),
  completed_at TIMESTAMP DEFAULT NOW(),
  UNIQUE(user_id, task_id)
);
