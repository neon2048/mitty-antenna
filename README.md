# Mitty Antenna

Deploy as Cloudflare worker:
```bash
# Create DB - you might have to update database_id in wrangler.toml
npx wrangler d1 create mitty-antenna-prod
# Apply SQL schema (Warning: drops DB if exists)
npx wrangler d1 execute mitty-antenna-prod --remote --file=./schema.sql

# upload secrets
# Webhook for the channel the bot should post to
npx wrangler secret put DISCORD_WEBHOOK 
# DISCORD_ROLE_ID should be the numerical Discord role ID, not just the name
npx wrangler secret put DISCORD_ROLE_ID 

# Deploy worker
npx wrangler deploy
```
