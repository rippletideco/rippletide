"""
Rippletide SDK Client for interacting with the Rippletide evaluation API.
"""
import os
import uuid
import random
import requests
from typing import Optional, Dict, List, Any, BinaryIO, Union
from pathlib import Path


class RippletideClient:
    """
    Client for interacting with the Rippletide evaluation API.
    
    Args:
        session_id: Optional session ID for anonymous requests (will be auto-generated if not provided and no api_key)
        api_key: Optional API key for authenticated requests
    """
    
    # Default base URL (override via constructor or RIPPLETIDE_BASE_URL)
    BASE_URL = "https://rippletide-backend.azurewebsites.net"

    def __init__(
        self,
        session_id: Optional[str] = None,
        api_key: Optional[str] = None,
        base_url: Optional[str] = None
    ):
        # Allow overriding base URL for staging/local via argument or env
        env_base_url = os.getenv("RIPPLETIDE_BASE_URL")
        self.base_url = (base_url or env_base_url or self.BASE_URL).rstrip('/')
        self.api_key = api_key
        
        # Generate session_id if not provided and no api_key
        if not api_key and not session_id:
            self.session_id = str(uuid.uuid4())
        else:
            self.session_id = session_id
        
        self.session = requests.Session()
        
        # Set up headers
        if self.api_key:
            self.session.headers.update({
                'x-api-key': self.api_key
            })
        if self.session_id:
            self.session.headers.update({
                'X-Session-Id': self.session_id
            })
    
    def _make_request(
        self,
        method: str,
        endpoint: str,
        **kwargs
    ) -> requests.Response:
        """
        Make an HTTP request to the API.
        
        Args:
            method: HTTP method (GET, POST, etc.)
            endpoint: API endpoint (e.g., '/api/agents')
            **kwargs: Additional arguments to pass to requests
            
        Returns:
            Response object
            
        Raises:
            requests.HTTPError: If the request fails
        """
        url = f"{self.base_url}{endpoint}"
        response = self.session.request(method, url, **kwargs)
        try:
            response.raise_for_status()
        except requests.HTTPError as e:
            # Include response body in error message for debugging
            error_msg = f"{e}\nResponse: {response.text}"
            raise requests.HTTPError(error_msg, response=response) from e
        return response
    
    def create_agent(
        self,
        name: str,
        seed: Optional[int] = None,
        num_nodes: int = 100,
        public_url: Optional[str] = None,
        advanced_payload: Optional[Dict[str, str]] = None,
        parent_agent_id: Optional[str] = None
    ) -> Dict[str, Any]:
        """
        Create a new agent for evaluation.
        
        Args:
            name: Name of the agent
            seed: Seed value for the agent (default: random)
            num_nodes: Number of nodes for the agent (default: 100)
            public_url: Optional public URL for the agent
            advanced_payload: Optional advanced payload configuration
            parent_agent_id: Optional parent agent ID
            
        Returns:
            Dictionary containing the created agent data
        """
        endpoint = '/api/agents/anonymous' if self.session_id and not self.api_key else '/api/agents'
        
        if seed is None:
            seed = random.randint(0, 1000000)
        
        payload = {
            'name': name,
            'seed': seed,
            'numNodes': num_nodes,
            'label': 'eval'
        }
        
        if public_url is not None:
            payload['publicUrl'] = public_url
        if advanced_payload is not None:
            payload['advancedPayload'] = advanced_payload
        if parent_agent_id is not None:
            payload['parentAgentId'] = parent_agent_id
        
        response = self._make_request('POST', endpoint, json=payload)
        return response.json()
    
    def extract_questions_from_pdf(
        self,
        agent_id: str,
        pdf_path: Union[str, Path, BinaryIO]
    ) -> Dict[str, Any]:
        """
        Extract questions and expected answers from a PDF file.
        
        Args:
            agent_id: ID of the agent
            pdf_path: Path to the PDF file or file-like object
            
        Returns:
            Dictionary containing extraction results and Q&A pairs
        """
        endpoint = f'/api/agents/{agent_id}/upload-pdf'
        
        # Handle both file path and file-like object
        if isinstance(pdf_path, (str, Path)):
            with open(pdf_path, 'rb') as f:
                files = {'file': (Path(pdf_path).name, f, 'application/pdf')}
                response = self._make_request('POST', endpoint, files=files)
        else:
            # Assume it's a file-like object
            files = {'file': ('document.pdf', pdf_path, 'application/pdf')}
            response = self._make_request('POST', endpoint, files=files)
        
        return response.json()
    
    def get_test_prompts(self, agent_id: str) -> List[Dict[str, Any]]:
        """
        Get all test prompts (questions and expected answers) for an agent.
        
        Args:
            agent_id: ID of the agent
            
        Returns:
            List of test prompts with question and expected answer
        """
        endpoint = f'/api/agents/{agent_id}/test-prompts'
        response = self._make_request('GET', endpoint)
        return response.json()
    
    def chat(
        self,
        agent_id: str,
        message: str
    ) -> Dict[str, Any]:
        """
        Send a chat message to an agent and get a response.
        This generates the response internally without using the agent's public URL.
        
        Args:
            agent_id: ID of the agent
            message: Message to send to the agent
            
        Returns:
            Dictionary containing agent response, session ID, etc.
        """
        endpoint = f'/api/agents/{agent_id}/chat'
        payload = {'message': message}
        response = self._make_request('POST', endpoint, json=payload)
        return response.json()
    
    def evaluate(
        self,
        agent_id: str,
        question: str,
        expected_answer: Optional[str] = None
    ) -> Dict[str, Any]:
        """
        Simple evaluation endpoint - evaluates a question and returns a report.
        
        Args:
            agent_id: ID of the agent
            question: The question to evaluate
            expected_answer: Optional expected answer (will use knowledge base if not provided)
            
        Returns:
            Dictionary containing evaluation report with label, justification, and facts
        """
        endpoint = f'/api/agents/{agent_id}/evaluate'
        payload = {'question': question}
        if expected_answer is not None:
            payload['expectedAnswer'] = expected_answer
        
        response = self._make_request('POST', endpoint, json=payload)
        return response.json()

