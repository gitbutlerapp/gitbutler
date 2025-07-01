cd /home/ec2-user
source .env

export NVM_DIR="/home/ec2-user/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"  # This loads nvm

while true; do
    cd /home/ec2-user
    source .env

    cd /home/ec2-user/gitbutler

    git pull

    pnpm install
    pnpm turbo package --filter=@gitbutler/butler-bot...

    cd /home/ec2-user/gitbutler/apps/butler-bot

    nvm install

    corepack enable
    corepack install

    pnpm prisma generate
    pnpm prisma migrate deploy

    pnpm run run
done
