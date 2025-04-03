cd /home/ec2-user
source .env

while true; do
    cd /home/ec2-user/gitbutler

    git pull

    pnpm install
    pnpm turbo package --filter=@gitbutler/butler-bot...

    cd /home/ec2-user/gitbutler/apps/butler-bot

    nvm install

    corepack enable
    corepack install

    pnpm run run
done
