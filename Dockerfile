FROM python:3.10.13-bullseye

WORKDIR /app

# copy dist of web
COPY ./web/dist /app/web/dist

COPY ./requirements.txt /app/requirements.txt
COPY ./main.py /app/main.py
COPY ./free_one_api /app/free_one_api

RUN pip install -r requirements.txt
RUN python main.py

CMD [ "python", "main.py" ]