"""
SDK Exceptions
"""


class TermSDKError(Exception):
    """Base exception for Term SDK"""
    pass


class CostLimitExceeded(TermSDKError):
    """Raised when cost limit is exceeded"""
    
    def __init__(self, message: str = "Cost limit exceeded"):
        super().__init__(message)
        self.message = message


class ProviderError(TermSDKError):
    """Raised when LLM provider returns an error"""
    
    def __init__(self, message: str, status_code: int = None, response: str = None):
        super().__init__(message)
        self.message = message
        self.status_code = status_code
        self.response = response


class ValidationError(TermSDKError):
    """Raised when agent validation fails"""
    
    def __init__(self, message: str):
        super().__init__(message)
        self.message = message


class TimeoutError(TermSDKError):
    """Raised when an operation times out"""
    
    def __init__(self, message: str = "Operation timed out"):
        super().__init__(message)
        self.message = message


class RateLimitError(TermSDKError):
    """Raised when rate limit is hit"""
    
    def __init__(self, message: str = "Rate limit exceeded", retry_after: float = None):
        super().__init__(message)
        self.message = message
        self.retry_after = retry_after
