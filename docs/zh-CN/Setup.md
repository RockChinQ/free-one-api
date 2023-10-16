# 安装

## Docker (推荐)

```bash
docker run -d -p 3000:3000 --restart always --name free-one-api -v ~/free-one-api/data:/app/data rockchin/free-one-api
```

此语句将启动 free-one-api 并指定 `~/free-one-api/data` 为容器的文件存储映射目录。  
你可以在 `http://localhost:3000/` 打开管理页面。

## 手动

```bash
git clone https://github.com/RockChinQ/free-one-api.git
cd free-one-api

cd web && npm install && npm run build && cd ..

pip install -r requirements.txt
python main.py
```

你可以在 `http://localhost:3000/` 打开管理页面。
