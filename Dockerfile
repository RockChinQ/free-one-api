FROM python:3.10.13-slim-bullseye

WORKDIR /app

# copy dist of web
COPY ./web/dist /app/web/dist
COPY ./free_one_api /app/free_one_api
COPY ./requirements.txt ./main.py /app/

RUN pip install --no-cache -r requirements.txt \
    && pip uninstall torch tensorflow transformers triton -y \
    && rm -rf /usr/local/lib/python3.10/site-packages/nvidia*

CMD [ "python", "main.py" ]