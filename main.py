import os
import sys
import asyncio

if not os.path.exists('./data'):
    os.mkdir('./data')

# change dir to ./data
os.chdir('./data')

from free_one_api.impls import app

def main():
    application = asyncio.run(app.make_application("./config.yml"))

    application.run()

if __name__ == "__main__":
    main()