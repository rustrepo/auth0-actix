

1. create a regular web app on Auth0.
2. When you create an app, you will get (on settings tab): domain (url in curl), client_id, client_secret. 
3. On settings tab/Allow callback url: set it to http://localhost:8080
4. On APIs tab: copy the API identifier, this is Audience: https://dev-abcdefg.us.auth0.com/api/v2/

Now to generate a bearer token, make a curl request from the terminal:

curl --request POST \
  --url https://YOUR_DOMAIN.auth0.com/oauth/token \
  --header 'content-type: application/json' \
  --data '{
    "client_id": "YOUR_CLIENT_ID",
    "client_secret": "YOUR_CLIENT_SECRET",
    "audience": "YOUR_API_IDENTIFIER",
    "grant_type": "client_credentials"
  }'

  here: 
  1. url = domain/oauth/token from settings tab
  2. client_id and client secret from the settings tab
  3. audience = from API tab- this is api indentifier.

  Then you will get a bearer token, which can be passed as authorisation in postman.

  1. Create users:
  Post: localhost:8080/users
  body: {"fullname: "James Bond}

    Response will give you a user id.

  2. Get user
  GET: localhost:8080/users/{id}

  3. PUT: 
  localhost:8080/users/{id}

  4. delete
  localhost:8080/users/{id}
