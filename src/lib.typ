#import "@preview/gentle-clues:1.2.0": *

#let card-container(
  title: "Anki", 
  icon: image("assets/anki.svg"),
  ..args
) = {
  clue(
    title: title,
    icon: icon,
    accent-color: rgb(4, 165, 229),
    ..args
  )
}

#let card(
  id: "",
  q: "",
  a: "",
  ..args
) = {
  let args = arguments(
    type: "basic",
    container: false,
    show-labels: false,
    ..args
  )

  if args.at("container") == false {
    if args.at("show-labels") == true {
      context [
        q: #q \ 
        a: #a
      ]
    } else {
      context [
        #q \
        #a
      ]
    }
  } else {
    if args.at("show-labels") == true {
      card-container[
        q: #q \ 
        a: #a
      ]
    } else {
      card-container[
        #q \ 
        #a
      ]
    }
  }
}

