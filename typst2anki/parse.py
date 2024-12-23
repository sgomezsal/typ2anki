def parse_cards(file_path, callback):
    inside_card = False
    balance = 0
    current_card = ""

    with open(file_path, "r") as file:
        file_content = file.read()

    i = 0
    while i < len(file_content):
        if not inside_card and file_content[i:i+6] == "#card(":
            inside_card = True
            balance = 1
            current_card = "#card("
            i += 6
            continue

        if inside_card:
            current_card += file_content[i]
            if file_content[i] == "(":
                balance += 1
            elif file_content[i] == ")":
                balance -= 1

            if balance == 0:
                callback(current_card.strip())
                inside_card = False
                current_card = ""

        i += 1
