

class QueryHandlingError(Exception):
    
    status_code: int
    code: str
    message: str
    type: str
    param: str
    
    def __init__(self, status_code: int, code: str, message: str, type: str=None, param: str=None):
        """Raise this exception when the query handling failed.
        
        Args:
            code (int): The http status code that should be returned to the client, please refer to the OpenAI API document.
            message (str): The error message that should be returned to the client.
            type (str): The error type that should be returned to the client.
            param (str): The error param that should be returned to the client.
        """
        self.status_code = status_code
        self.code = code
        self.message = message
        self.type = type
        self.param = param
