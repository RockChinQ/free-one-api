# Setup

## Docker (Recommended)

```bash
docker run -d -p 3000:3000 --restart always --name free-one-api -v ~/free-one-api/data:/app/data rockchin/free-one-api
```

This command will start free-one-api and specify `~/free-one-api/data` as the container's file storage mapping directory.  
Then you can open the admin page at `http://localhost:3000/`.

### Docker Compose Example

If you have installed Docker Compose, you can use the following example to start free-one-api:

```yaml
version: '3'
services:
  free-one-api:
    container_name: free-one-api
    image: rockchin/free-one-api
    ports:
      - "3000:3000"
    restart: always
    volumes:
      - ~/free-one-api/data:/app/data
```

Save as `docker-compose.yaml`, then execute the following command in the directory where the file is located:

```bash
docker-compose up -d
```

## Manual

```bash
git clone https://github.com/RockChinQ/free-one-api.git
cd free-one-api

cd web && npm install && npm run build && cd ..

pip install -r requirements.txt
python main.py
```

then you can open the admin page at `http://localhost:3000/`.