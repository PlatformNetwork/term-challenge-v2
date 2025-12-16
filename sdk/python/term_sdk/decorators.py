"""
Decorators for agent methods
"""

import asyncio
import functools
import time
from typing import Any, Callable, Optional, TypeVar, Union

F = TypeVar("F", bound=Callable[..., Any])


def tool(
    name: Optional[str] = None,
    description: Optional[str] = None,
) -> Callable[[F], F]:
    """
    Decorator to mark a method as a tool.
    
    Tools can be called by the agent during task execution.
    
    Usage:
        class MyAgent(Agent):
            @tool(description="Search the web")
            async def search(self, query: str) -> str:
                ...
    """
    def decorator(func: F) -> F:
        func._is_tool = True
        func._tool_name = name or func.__name__
        func._tool_description = description or func.__doc__ or ""
        return func
    
    return decorator


def with_retry(
    max_attempts: int = 3,
    delay: float = 1.0,
    backoff: float = 2.0,
    exceptions: tuple = (Exception,),
) -> Callable[[F], F]:
    """
    Decorator to retry a function on failure.
    
    Usage:
        @with_retry(max_attempts=3, delay=1.0)
        async def call_api():
            ...
    
    Args:
        max_attempts: Maximum number of attempts
        delay: Initial delay between attempts (seconds)
        backoff: Multiplier for delay after each attempt
        exceptions: Exception types to catch and retry
    """
    def decorator(func: F) -> F:
        @functools.wraps(func)
        async def async_wrapper(*args, **kwargs):
            last_exception = None
            current_delay = delay
            
            for attempt in range(max_attempts):
                try:
                    return await func(*args, **kwargs)
                except exceptions as e:
                    last_exception = e
                    if attempt < max_attempts - 1:
                        await asyncio.sleep(current_delay)
                        current_delay *= backoff
            
            raise last_exception
        
        @functools.wraps(func)
        def sync_wrapper(*args, **kwargs):
            last_exception = None
            current_delay = delay
            
            for attempt in range(max_attempts):
                try:
                    return func(*args, **kwargs)
                except exceptions as e:
                    last_exception = e
                    if attempt < max_attempts - 1:
                        time.sleep(current_delay)
                        current_delay *= backoff
            
            raise last_exception
        
        if asyncio.iscoroutinefunction(func):
            return async_wrapper
        return sync_wrapper
    
    return decorator


def rate_limit(
    calls: int = 10,
    period: float = 60.0,
) -> Callable[[F], F]:
    """
    Decorator to rate limit function calls.
    
    Usage:
        @rate_limit(calls=10, period=60)  # 10 calls per minute
        async def call_api():
            ...
    
    Args:
        calls: Maximum number of calls allowed
        period: Time period in seconds
    """
    def decorator(func: F) -> F:
        call_times = []
        lock = asyncio.Lock()
        
        @functools.wraps(func)
        async def async_wrapper(*args, **kwargs):
            async with lock:
                now = time.time()
                
                # Remove old calls
                while call_times and now - call_times[0] > period:
                    call_times.pop(0)
                
                # Check rate limit
                if len(call_times) >= calls:
                    wait_time = period - (now - call_times[0])
                    if wait_time > 0:
                        await asyncio.sleep(wait_time)
                        # Remove old calls again after waiting
                        now = time.time()
                        while call_times and now - call_times[0] > period:
                            call_times.pop(0)
                
                call_times.append(now)
            
            return await func(*args, **kwargs)
        
        @functools.wraps(func)
        def sync_wrapper(*args, **kwargs):
            now = time.time()
            
            # Remove old calls
            while call_times and now - call_times[0] > period:
                call_times.pop(0)
            
            # Check rate limit
            if len(call_times) >= calls:
                wait_time = period - (now - call_times[0])
                if wait_time > 0:
                    time.sleep(wait_time)
                    now = time.time()
                    while call_times and now - call_times[0] > period:
                        call_times.pop(0)
            
            call_times.append(now)
            return func(*args, **kwargs)
        
        if asyncio.iscoroutinefunction(func):
            return async_wrapper
        return sync_wrapper
    
    return decorator


def timeout(seconds: float) -> Callable[[F], F]:
    """
    Decorator to add timeout to async functions.
    
    Usage:
        @timeout(30)  # 30 second timeout
        async def long_operation():
            ...
    
    Args:
        seconds: Timeout in seconds
    """
    def decorator(func: F) -> F:
        @functools.wraps(func)
        async def wrapper(*args, **kwargs):
            return await asyncio.wait_for(
                func(*args, **kwargs),
                timeout=seconds,
            )
        return wrapper
    
    return decorator


def cache(ttl: Optional[float] = None, maxsize: int = 128) -> Callable[[F], F]:
    """
    Simple async cache decorator.
    
    Usage:
        @cache(ttl=60)  # Cache for 60 seconds
        async def expensive_operation(key: str):
            ...
    
    Args:
        ttl: Time to live in seconds (None for no expiration)
        maxsize: Maximum cache size
    """
    def decorator(func: F) -> F:
        _cache = {}
        _timestamps = {}
        
        @functools.wraps(func)
        async def wrapper(*args, **kwargs):
            # Create cache key
            key = (args, tuple(sorted(kwargs.items())))
            
            # Check cache
            if key in _cache:
                if ttl is None or (time.time() - _timestamps[key]) < ttl:
                    return _cache[key]
            
            # Call function
            result = await func(*args, **kwargs)
            
            # Store in cache
            if len(_cache) >= maxsize:
                # Remove oldest entry
                oldest_key = min(_timestamps, key=_timestamps.get)
                del _cache[oldest_key]
                del _timestamps[oldest_key]
            
            _cache[key] = result
            _timestamps[key] = time.time()
            
            return result
        
        wrapper.cache_clear = lambda: (_cache.clear(), _timestamps.clear())
        return wrapper
    
    return decorator
