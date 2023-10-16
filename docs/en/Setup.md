# Setup

## Docker (Recommended)

```bash
docker run -d -p 3000:3000 --restart always --name free-one-api -v ~/free-one-api/data:/app/data rockchin/free-one-api
```

This command will start free-one-api and specify `~/free-one-api/data` as the container's file storage mapping directory.  
Then you can open the admin page at `http://localhost:3000/`.

## Manual

```bash
git clone https://github.com/RockChinQ/free-one-api.git
cd free-one-api

cd web && npm install && npm run build && cd ..

pip install -r requirements.txt
python main.py
```

then you can open the admin page at `http://localhost:3000/`.