import random
import typing

ads = []

rate = 0.01

enabled = False

def generate_ad() -> typing.Generator[str, None, None]:
    """Generate random ad."""
    global ads
    global rate
    global enabled
    
    if not enabled:
        return
    
    if len(ads) == 0:
        return
    
    if random.random() < rate:
        ad_words = random.choice(ads).split(" ")
        
        for word in ad_words:
            yield word
            yield " "
