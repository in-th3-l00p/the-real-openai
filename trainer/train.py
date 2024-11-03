import tensorflow as tf
from tensorflow import keras
from tensorflow.keras import layers

def train():
    # Load the MNIST dataset
    (x_train, y_train), (x_test, y_test) = keras.datasets.mnist.load_data()

    # Normalize pixel values to [0, 1] and reshape the data
    x_train = x_train.astype("float32") / 255.0
    x_train = x_train[..., tf.newaxis]  # Add channel dimension

    x_test = x_test.astype("float32") / 255.0
    x_test = x_test[..., tf.newaxis]    # Add channel dimension

    # Build the model
    model = keras.Sequential([
        layers.Conv2D(32, kernel_size=(3, 3), activation='relu', input_shape=(28, 28, 1)),
        layers.MaxPooling2D(pool_size=(2, 2)),
        layers.Conv2D(64, kernel_size=(3, 3), activation='relu'),
        layers.MaxPooling2D(pool_size=(2, 2)),
        layers.Flatten(),
        layers.Dense(128, activation='relu'),
        layers.Dense(10, activation='softmax'),
    ])

    # Compile the model
    model.compile(optimizer='adam',
                loss='sparse_categorical_crossentropy',
                metrics=['accuracy'])

    # Train the model
    print(x_train[0])
    model.fit(
        x_train, y_train,
        epochs=5,
        batch_size=64,
        validation_split=0.1
    )

    # Save the trained model
    model.save('../api/model.keras')

train()