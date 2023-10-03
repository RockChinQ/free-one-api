import os
import sys
import asyncio

if not os.path.exists('./data'):
    os.mkdir('./data')

from free_one_api.impls import app

def main():
    application = asyncio.run(app.make_application("./data/config.yaml"))

    application.run()

if __name__ == "__main__":
    main()
