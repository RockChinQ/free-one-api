import os
import sys
import asyncio
import logging

logging.basicConfig(level=logging.INFO)

if not os.path.exists('./data'):
    os.mkdir('./data')

from free_one_api.impls import app

def main():
    loop = asyncio.get_event_loop()
    
    application = loop.run_until_complete(app.make_application("./data/config.yaml"))

    loop.run_until_complete(application.run())

if __name__ == "__main__":
    main()
