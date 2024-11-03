from flask import Flask, request, jsonify
from flask_cors import CORS
import tensorflow as tf
import numpy as np

app = Flask(__name__)

# Configure CORS to allow requests from Vite's localhost
CORS(app, resources={r"/predict": {"origins": "http://localhost:5173"}})

# Load the MNIST model
model = tf.keras.models.load_model('model.keras')

@app.route('/predict', methods=['POST'])
def predict():
    try:
        # Get JSON data from the request
        data = request.get_json(force=True)

        # Convert the input data into a NumPy array
        input_data = np.array(data)

        # Validate input shape
        if input_data.shape != (28, 28):
            return jsonify({'error': 'Invalid input shape. Expected a 28x28 matrix.'}), 400

        # Preprocess the input data
        input_data = input_data.reshape(1, 28, 28, 1)  # Add batch and channel dimensions
        input_data = input_data.astype('float32') # Normalize pixel values

        # Make a prediction
        predictions = model.predict(input_data)
        predicted_class = int(np.argmax(predictions, axis=1)[0])

        # Return the prediction as JSON
        return jsonify({'prediction': predicted_class})

    except Exception as e:
        return jsonify({'error': str(e)}), 500

if __name__ == '__main__':
    app.run(debug=True)
